use sea_orm::{
    sea_query::{Expr, SimpleExpr, Value},
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};

use crate::enums::{DraftIssueStatus, ItemStatus, ItemType};
use crate::models::item;

/// Parameters for querying items.
#[derive(Debug, Default)]
pub struct ListItemsParams {
    pub project_id: Option<String>,
    pub item_type: Option<String>,
    pub starred_only: bool,
    pub search_query: Option<String>,
    pub cursor: Option<String>,
    pub page_size: u32,
    /// When true, returns dismissed items; when false, returns non-dismissed items.
    pub dismissed: bool,
}

/// Paginated response from item queries.
#[derive(Debug)]
pub struct ItemPage {
    pub items: Vec<item::Model>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// List items with filtering, search, and cursor-based pagination.
pub async fn list_items(
    db: &DatabaseConnection,
    params: ListItemsParams,
) -> Result<ItemPage, sea_orm::DbErr> {
    let limit = params.page_size.min(200) as u64;

    let mut query = item::Entity::find()
        .order_by_desc(item::Column::UpdatedAt)
        .order_by_desc(item::Column::Id);

    // Filter by item_status: dismissed tab shows dismissed items, inbox shows pending items
    if params.dismissed {
        query = query.filter(item::Column::ItemStatus.eq(ItemStatus::Dismissed));
    } else {
        query = query.filter(item::Column::ItemStatus.eq(ItemStatus::Pending));
    }

    if let Some(pid) = params.project_id {
        query = query.filter(item::Column::ProjectId.eq(pid));
    }

    if let Some(ref itype) = params.item_type {
        if itype != "all" {
            query = query.filter(item::Column::ItemType.eq(itype.as_str()));
        }
    }

    if params.starred_only {
        query = query.filter(item::Column::IsStarred.eq(true));
    }

    if let Some(ref q) = params.search_query {
        let q = q.trim();
        if !q.is_empty() {
            let pattern = format!("%{}%", q);
            let json_like = |path: &str, pat: &str| -> SimpleExpr {
                Expr::cust_with_values::<_, Value, _>(
                    &format!("json_extract(type_data, '{path}') LIKE ?"),
                    [pat.into()],
                )
            };
            let mut search_cond = Condition::any()
                .add(item::Column::Title.like(&pattern))
                .add(item::Column::Body.like(&pattern))
                // Use json_extract to search within specific JSON fields,
                // preventing cross-field LIKE matching
                .add(json_like("$.author", &pattern))
                .add(json_like("$.labels", &pattern))
                .add(json_like("$.pr_branch", &pattern));
            // If the query looks like a number, also match external_id in type_data JSON
            if let Ok(num) = q.trim_start_matches('#').parse::<i64>() {
                search_cond = search_cond.add(Expr::cust_with_values::<_, Value, _>(
                    "json_extract(type_data, '$.external_id') = ?",
                    [num.into()],
                ));
            }
            query = query.filter(search_cond);
        }
    }

    // Cursor-based pagination: cursor = "updated_at|id"
    if let Some(ref cursor_str) = params.cursor {
        if let Some((ts_str, cursor_id)) = cursor_str.split_once('|') {
            if let Ok(ts) = chrono::NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d %H:%M:%S%.f") {
                query = query.filter(
                    Condition::any().add(item::Column::UpdatedAt.lt(ts)).add(
                        Condition::all()
                            .add(item::Column::UpdatedAt.eq(ts))
                            .add(item::Column::Id.lt(cursor_id.to_string())),
                    ),
                );
            }
        }
    }

    // Fetch one extra to determine if there are more
    let items = query.limit(limit + 1).all(db).await?;

    let has_more = items.len() as u64 > limit;
    let mut items: Vec<item::Model> = items.into_iter().take(limit as usize).collect();

    // Post-filter: exclude submitted notes
    if !params.dismissed {
        items.retain(|i| {
            if i.item_type == ItemType::Note {
                if let Ok(td) = i.parse_type_data() {
                    td.draft_status() != Some(&DraftIssueStatus::Submitted)
                } else {
                    true
                }
            } else {
                true
            }
        });
    }

    let next_cursor = if has_more {
        items.last().map(|last| {
            format!(
                "{}|{}",
                last.updated_at.format("%Y-%m-%d %H:%M:%S%.f"),
                last.id
            )
        })
    } else {
        None
    };

    Ok(ItemPage {
        items,
        next_cursor,
        has_more,
    })
}

/// Count dismissed items grouped by (project_id, item_type).
pub async fn count_dismissed_grouped(
    db: &DatabaseConnection,
) -> Result<Vec<DismissedCount>, sea_orm::DbErr> {
    use sea_orm::FromQueryResult;

    #[derive(Debug, FromQueryResult)]
    struct Row {
        project_id: String,
        item_type: String,
        count: i64,
    }

    let rows = item::Entity::find()
        .filter(item::Column::ItemStatus.eq(ItemStatus::Dismissed))
        .select_only()
        .column(item::Column::ProjectId)
        .column(item::Column::ItemType)
        .column_as(item::Column::Id.count(), "count")
        .group_by(item::Column::ProjectId)
        .group_by(item::Column::ItemType)
        .into_model::<Row>()
        .all(db)
        .await?;

    Ok(rows
        .into_iter()
        .map(|r| DismissedCount {
            project_id: r.project_id,
            item_type: r.item_type,
            count: r.count as u64,
        })
        .collect())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DismissedCount {
    pub project_id: String,
    pub item_type: String,
    pub count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::{ItemStatus, ItemType};
    use crate::test_helpers::*;

    async fn setup() -> (sea_orm::DatabaseConnection, crate::models::project::Model) {
        let db = setup_test_db().await;
        let connector = ConnectorFactory::default().create(&db).await;
        let project = ProjectFactory::default()
            .connector_id(&connector.id)
            .create(&db)
            .await;
        (db, project)
    }

    #[tokio::test]
    async fn search_by_title() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Fix authentication bug")
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Add new feature")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("authentication".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Fix authentication bug");
    }

    #[tokio::test]
    async fn search_by_body() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Issue A")
            .body("This involves the payment gateway")
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Issue B")
            .body("Simple UI change")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("payment".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Issue A");
    }

    #[tokio::test]
    async fn search_by_external_id_numeric() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 42)
            .title("Some issue")
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 99)
            .title("Other issue")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("42".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].title.contains("Some issue"));
    }

    #[tokio::test]
    async fn search_by_external_id_with_hash() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 42)
            .title("Some issue")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("#42".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
    }

    #[tokio::test]
    async fn search_by_author() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Issue from alice")
            .author("alice")
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Issue from bob")
            .author("bob")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("alice".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Issue from alice");
    }

    #[tokio::test]
    async fn search_by_label() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Bug report")
            .labels(vec!["bug", "critical"])
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Feature request")
            .labels(vec!["enhancement"])
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("critical".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Bug report");
    }

    #[tokio::test]
    async fn search_by_pr_branch() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Feature PR")
            .item_type(ItemType::PullRequest)
            .pr_branch("feat/user-auth")
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Bugfix PR")
            .item_type(ItemType::PullRequest)
            .pr_branch("fix/login")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("user-auth".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Feature PR");
    }

    #[tokio::test]
    async fn search_empty_query_returns_all() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1).create(&db).await;
        ItemFactory::new(&project.id, 2).create(&db).await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("  ".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 2);
    }

    #[tokio::test]
    async fn search_no_match() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Something")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("nonexistent".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 0);
    }

    #[tokio::test]
    async fn search_respects_item_type_filter() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Auth issue")
            .item_type(ItemType::Issue)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Auth PR")
            .item_type(ItemType::PullRequest)
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("Auth".to_string()),
                item_type: Some("issue".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Auth issue");
    }

    #[tokio::test]
    async fn search_respects_starred_filter() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Starred bug")
            .is_starred(true)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Unstarred bug")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("bug".to_string()),
                starred_only: true,
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Starred bug");
    }

    #[tokio::test]
    async fn search_excludes_non_pending_items() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Visible bug")
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Resolved bug")
            .item_status(ItemStatus::Resolved)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 3)
            .title("Deleted bug")
            .item_status(ItemStatus::Deleted)
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("bug".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Visible bug");
    }

    #[tokio::test]
    async fn search_dismissed_items() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Active bug")
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Dismissed bug")
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(now())
            .create(&db)
            .await;

        // Non-dismissed search should not find dismissed item
        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("bug".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Active bug");

        // Dismissed search should only find dismissed item
        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("bug".to_string()),
                dismissed: true,
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Dismissed bug");
    }

    #[tokio::test]
    async fn search_respects_project_filter() {
        let db = setup_test_db().await;
        let connector = ConnectorFactory::default().create(&db).await;
        let project_a = ProjectFactory::default()
            .name("project-a")
            .connector_id(&connector.id)
            .create(&db)
            .await;
        let project_b = ProjectFactory::default()
            .name("project-b")
            .connector_id(&connector.id)
            .create(&db)
            .await;

        ItemFactory::new(&project_a.id, 1)
            .title("Bug in A")
            .create(&db)
            .await;
        ItemFactory::new(&project_b.id, 2)
            .title("Bug in B")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("Bug".to_string()),
                project_id: Some(project_a.id.clone()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Bug in A");
    }

    #[tokio::test]
    async fn search_matches_multiple_fields() {
        let (db, project) = setup().await;
        // This item matches via title
        ItemFactory::new(&project.id, 1)
            .title("Fix login")
            .body("unrelated body")
            .author("bob")
            .create(&db)
            .await;
        // This item matches via author
        ItemFactory::new(&project.id, 2)
            .title("Unrelated title")
            .body("unrelated body")
            .author("login-bot")
            .create(&db)
            .await;
        // This item doesn't match
        ItemFactory::new(&project.id, 3)
            .title("Other")
            .body("nothing here")
            .author("charlie")
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                search_query: Some("login".to_string()),
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 2);
    }

    #[tokio::test]
    async fn pagination_with_search() {
        let (db, project) = setup().await;
        for i in 1..=5 {
            ItemFactory::new(&project.id, i)
                .title(&format!("Bug #{}", i))
                .updated_at(dt(&format!("2024-01-{:02} 10:00:00", i)))
                .create(&db)
                .await;
        }

        // First page
        let page1 = list_items(
            &db,
            ListItemsParams {
                search_query: Some("Bug".to_string()),
                page_size: 2,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(page1.items.len(), 2);
        assert!(page1.has_more);
        assert!(page1.next_cursor.is_some());

        // Second page
        let page2 = list_items(
            &db,
            ListItemsParams {
                search_query: Some("Bug".to_string()),
                cursor: page1.next_cursor,
                page_size: 2,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(page2.items.len(), 2);
        assert!(page2.has_more);

        // Third page
        let page3 = list_items(
            &db,
            ListItemsParams {
                search_query: Some("Bug".to_string()),
                cursor: page2.next_cursor,
                page_size: 2,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(page3.items.len(), 1);
        assert!(!page3.has_more);
    }

    #[tokio::test]
    async fn list_items_returns_only_pending() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Pending")
            .item_status(ItemStatus::Pending)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Resolved")
            .item_status(ItemStatus::Resolved)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 3)
            .title("Dismissed")
            .item_status(ItemStatus::Dismissed)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 4)
            .title("Deleted")
            .item_status(ItemStatus::Deleted)
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Pending");
    }

    #[tokio::test]
    async fn list_dismissed_items_returns_only_dismissed() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Pending")
            .item_status(ItemStatus::Pending)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .title("Dismissed")
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(now())
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 3)
            .title("Resolved")
            .item_status(ItemStatus::Resolved)
            .create(&db)
            .await;

        let result = list_items(
            &db,
            ListItemsParams {
                dismissed: true,
                page_size: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Dismissed");
    }

    #[tokio::test]
    async fn count_dismissed_grouped_by_project_and_type() {
        let db = setup_test_db().await;
        let connector = ConnectorFactory::default().create(&db).await;
        let project_a = ProjectFactory::default()
            .name("project-a")
            .connector_id(&connector.id)
            .create(&db)
            .await;
        let project_b = ProjectFactory::default()
            .name("project-b")
            .connector_id(&connector.id)
            .create(&db)
            .await;

        // Project A: 2 dismissed issues, 1 dismissed PR, 1 pending
        ItemFactory::new(&project_a.id, 1)
            .item_type(ItemType::Issue)
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(now())
            .create(&db)
            .await;
        ItemFactory::new(&project_a.id, 2)
            .item_type(ItemType::Issue)
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(now())
            .create(&db)
            .await;
        ItemFactory::new(&project_a.id, 3)
            .item_type(ItemType::PullRequest)
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(now())
            .create(&db)
            .await;
        ItemFactory::new(&project_a.id, 4)
            .item_type(ItemType::Issue)
            .create(&db)
            .await;

        // Project B: 1 dismissed issue
        ItemFactory::new(&project_b.id, 5)
            .item_type(ItemType::Issue)
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(now())
            .create(&db)
            .await;

        let counts = count_dismissed_grouped(&db).await.unwrap();

        // Should have 3 groups: (A, issue), (A, pr), (B, issue)
        assert_eq!(counts.len(), 3);

        let a_issues: u64 = counts
            .iter()
            .filter(|c| c.project_id == project_a.id && c.item_type == "issue")
            .map(|c| c.count)
            .sum();
        assert_eq!(a_issues, 2);

        let a_prs: u64 = counts
            .iter()
            .filter(|c| c.project_id == project_a.id && c.item_type == "pr")
            .map(|c| c.count)
            .sum();
        assert_eq!(a_prs, 1);

        let b_issues: u64 = counts
            .iter()
            .filter(|c| c.project_id == project_b.id && c.item_type == "issue")
            .map(|c| c.count)
            .sum();
        assert_eq!(b_issues, 1);
    }

    #[tokio::test]
    async fn count_dismissed_grouped_empty_when_none_dismissed() {
        let (db, project) = setup().await;
        ItemFactory::new(&project.id, 1)
            .title("Pending item")
            .create(&db)
            .await;

        let counts = count_dismissed_grouped(&db).await.unwrap();
        assert!(counts.is_empty());
    }
}
