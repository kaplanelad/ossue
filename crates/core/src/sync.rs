use chrono::NaiveDateTime;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set, TransactionTrait,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::enums::{ItemState, ItemStatus, ItemType, ItemTypeData, PrItemData, ProviderItemData};
use crate::models::item;
use crate::models::project;

/// Index of existing items keyed by `(external_id, item_type)` for fast
/// duplicate detection during sync.
pub type ItemsIndex = HashMap<(i32, ItemType), item::Model>;

/// Loads all items for a project into a [`ItemsIndex`] so the sync
/// pipeline can quickly decide whether an incoming API item is new or existing.
/// Includes deleted items so that re-synced closed items update existing rows
/// instead of creating duplicates.
#[tracing::instrument(skip(db))]
pub async fn load_items_index(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<ItemsIndex, sea_orm::DbErr> {
    let items = item::Entity::find()
        .filter(item::Column::ProjectId.eq(project_id))
        .filter(item::Column::ItemType.ne("note"))
        .all(db)
        .await?;

    let mut index = HashMap::with_capacity(items.len());
    for item in items {
        if let Ok(td) = item.parse_type_data() {
            if let Some(ext_id) = td.external_id() {
                index.insert((ext_id, item.item_type.clone()), item);
            }
        }
    }
    Ok(index)
}

/// A platform-agnostic representation of an item to upsert during sync.
pub struct NewItem {
    pub external_id: i32,
    pub item_type: ItemType,
    pub title: String,
    pub body: String,
    pub state: ItemState,
    pub author: String,
    pub url: String,
    pub comments_count: i32,
    pub pr_branch: Option<String>,
    pub labels: Vec<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Upserts a batch of items for a given project.
///
/// Items that already exist in `items_index` are updated; new items are
/// inserted. When an existing item has a newer `updated_at` than the stored
/// record, the item is also marked as unread so the user notices the new
/// activity.
///
/// Newly inserted items are added to `items_index` so that subsequent pages
/// within the same sync run do not attempt to re-insert them.
///
/// Returns all saved (inserted or updated) item models.
#[tracing::instrument(skip(db, items_index, new_items), fields(batch_size = new_items.len()))]
pub async fn upsert_items_batch(
    db: &DatabaseConnection,
    project_id: &str,
    items_index: &mut ItemsIndex,
    new_items: Vec<NewItem>,
) -> Result<Vec<item::Model>, sea_orm::DbErr> {
    let now = chrono::Utc::now().naive_utc();
    let mut saved: Vec<item::Model> = Vec::with_capacity(new_items.len());

    let txn = db.begin().await?;

    for new_item in new_items {
        let key = (new_item.external_id, new_item.item_type.clone());

        if let Some(existing) = items_index.get(&key) {
            // --- Update existing item ---
            let has_new_activity = new_item.updated_at > existing.updated_at;

            let is_closed_or_merged =
                matches!(new_item.state, ItemState::Closed | ItemState::Merged);

            // Parse existing type_data, update fields, re-serialize
            let type_data = build_type_data(&new_item, &now)?;

            let mut active: item::ActiveModel = existing.clone().into();
            active.title = Set(new_item.title);
            active.body = Set(new_item.body);
            active.type_data = Set(type_data);
            active.updated_at = Set(new_item.updated_at);

            // Determine item_status based on priority rules:
            // 1. User-deleted items stay deleted (permanent)
            // 2. Provider closed/merged → resolved (provider wins over dismiss)
            // 3. New activity on dismissed item → pending (auto-undismiss)
            // 4. Dismissed items with no new activity stay dismissed
            // 5. Otherwise → pending for open, resolved for closed
            if existing.item_status == ItemStatus::Deleted {
                // User delete is permanent — don't change status
            } else if is_closed_or_merged {
                active.item_status = Set(ItemStatus::Resolved);
            } else if has_new_activity && existing.item_status == ItemStatus::Dismissed {
                active.item_status = Set(ItemStatus::Pending);
                active.is_read = Set(false);
            } else if existing.item_status != ItemStatus::Dismissed {
                active.item_status = Set(ItemStatus::Pending);
            }

            if has_new_activity && existing.item_status != ItemStatus::Deleted {
                active.is_read = Set(false);
            }

            let model = active.update(&txn).await?;

            // Keep the index up-to-date with the latest version.
            items_index.insert(key, model.clone());
            saved.push(model);
        } else {
            // --- Insert new item ---
            // Skip items that are already closed/merged and not in our DB.
            // During incremental sync with state=all, we only want to track
            // items that were previously open.
            if matches!(new_item.state, ItemState::Closed | ItemState::Merged) {
                continue;
            }
            let id = Uuid::new_v4().to_string();

            let type_data = build_type_data(&new_item, &now)?;

            let active = item::ActiveModel {
                id: Set(id),
                project_id: Set(project_id.to_string()),
                item_type: Set(new_item.item_type),
                title: Set(new_item.title),
                body: Set(new_item.body),
                type_data: Set(type_data),
                is_read: Set(false),
                is_starred: Set(false),
                is_deleted: Set(false),
                item_status: Set(ItemStatus::Pending),
                dismissed_at: Set(None),
                created_at: Set(new_item.created_at),
                updated_at: Set(new_item.updated_at),
            };

            let model = active.insert(&txn).await?;

            // Add to index so later pages in the same sync don't re-insert.
            if let Ok(td) = model.parse_type_data() {
                if let Some(ext_id) = td.external_id() {
                    items_index.insert((ext_id, model.item_type.clone()), model.clone());
                }
            }
            saved.push(model);
        }
    }

    txn.commit().await?;

    Ok(saved)
}

fn build_type_data(new_item: &NewItem, now: &NaiveDateTime) -> Result<String, sea_orm::DbErr> {
    let provider = ProviderItemData {
        external_id: new_item.external_id,
        state: new_item.state.clone(),
        author: new_item.author.clone(),
        url: new_item.url.clone(),
        comments_count: new_item.comments_count,
        fetched_at: now.to_string(),
        labels: new_item.labels.clone(),
    };

    let data = match new_item.item_type {
        ItemType::PullRequest => ItemTypeData::Pr(PrItemData {
            provider,
            pr_branch: new_item.pr_branch.clone(),
            pr_diff: None,
        }),
        ItemType::Discussion => ItemTypeData::Discussion(provider),
        _ => ItemTypeData::Issue(provider),
    };

    serde_json::to_string(&data).map_err(|e| sea_orm::DbErr::Custom(e.to_string()))
}

/// Sets `project.last_sync_at` to the current time so the UI shows when the
/// sync actually ran. Also clears `last_sync_error` on success.
#[tracing::instrument(skip(db, project), fields(project_id = %project.id))]
pub async fn advance_sync_timestamp(
    db: &DatabaseConnection,
    project: &project::Model,
) -> Result<(), sea_orm::DbErr> {
    let mut active: project::ActiveModel = project.clone().into();
    active.last_sync_at = Set(Some(chrono::Utc::now().naive_utc()));
    active.last_sync_error = Set(None);
    active.update(db).await?;

    Ok(())
}

/// Marks items of the given type that were NOT returned by the API as closed.
///
/// This is used during full reconciliation to detect items that have been
/// removed or closed on the remote side but were not included in the API
/// response.
///
/// Uses raw SQL because `state` lives inside the JSON `type_data` column.
#[tracing::instrument(skip(db, fetched_external_ids), fields(fetched_count = fetched_external_ids.len()))]
pub async fn mark_absent_items_closed(
    db: &DatabaseConnection,
    project_id: &str,
    fetched_external_ids: &[i32],
    item_type: &ItemType,
) -> Result<(), sea_orm::DbErr> {
    // For PRs, serde(flatten) means state/external_id are at top level alongside pr_branch
    let state_path = "$.state";
    let ext_id_path = "$.external_id";

    let mut params: Vec<sea_orm::Value> = vec![project_id.into(), item_type.to_string().into()];

    let not_in = if fetched_external_ids.is_empty() {
        String::new()
    } else {
        let placeholders: Vec<String> = fetched_external_ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                params.push((*id).into());
                format!("?{}", i + 3) // ?1 and ?2 are project_id and item_type
            })
            .collect();
        format!(
            " AND json_extract(type_data, '{ext_id_path}') NOT IN ({})",
            placeholders.join(",")
        )
    };

    let sql = format!(
        "UPDATE items SET type_data = json_set(type_data, '{state_path}', 'closed'), item_status = 'resolved' \
         WHERE project_id = ?1 AND item_type = ?2 AND item_status != 'resolved' AND item_status != 'deleted' \
         AND json_extract(type_data, '{ext_id_path}') IS NOT NULL{not_in}"
    );

    db.execute(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        &sql,
        params,
    ))
    .await?;

    Ok(())
}

/// Ensures any item whose JSON state is closed/merged has `item_status = 'resolved'`.
///
/// This catches items that ended up with `state = closed/merged` but
/// `item_status = 'pending'` — for example after a full sync resets statuses
/// and incremental sync doesn't re-fetch old closed items.
#[tracing::instrument(skip(db), fields(project_id = %project_id))]
pub async fn deactivate_closed_items(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<(), sea_orm::DbErr> {
    let sql = "UPDATE items SET item_status = 'resolved' \
               WHERE project_id = ?1 AND item_status != 'resolved' AND item_status != 'deleted' \
               AND (json_extract(type_data, '$.state') = 'closed' \
                    OR json_extract(type_data, '$.state') = 'merged')";

    db.execute(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        sql,
        vec![project_id.into()],
    ))
    .await?;

    Ok(())
}

/// Sets `project.full_reconciliation_at` to the current UTC time.
#[tracing::instrument(skip(db, project), fields(project_id = %project.id))]
pub async fn update_reconciliation_timestamp(
    db: &DatabaseConnection,
    project: &project::Model,
) -> Result<(), sea_orm::DbErr> {
    let mut active: project::ActiveModel = project.clone().into();
    active.full_reconciliation_at = Set(Some(chrono::Utc::now().naive_utc()));
    active.update(db).await?;

    Ok(())
}

/// Records a sync error on the project so the UI can display it.
#[tracing::instrument(skip(db, project), fields(project_id = %project.id))]
pub async fn set_sync_error(
    db: &DatabaseConnection,
    project: &project::Model,
    error_message: &str,
) -> Result<(), sea_orm::DbErr> {
    let mut active: project::ActiveModel = project.clone().into();
    active.last_sync_error = Set(Some(error_message.to_string()));
    active.update(db).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use rstest::rstest;
    use sea_orm::EntityTrait;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    async fn setup() -> (DatabaseConnection, project::Model) {
        let db = setup_test_db().await;
        let connector = ConnectorFactory::default().create(&db).await;
        let project = ProjectFactory::default()
            .connector_id(&connector.id)
            .create(&db)
            .await;
        (db, project)
    }

    // -----------------------------------------------------------------------
    // load_items_index
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn load_items_index_empty_project() {
        let (db, project) = setup().await;
        let index = load_items_index(&db, &project.id).await.unwrap();
        assert!(index.is_empty());
    }

    #[tokio::test]
    async fn load_items_index_returns_all_items() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1).create(&db).await;
        ItemFactory::new(&project.id, 2).create(&db).await;
        ItemFactory::new(&project.id, 3)
            .is_deleted(true)
            .create(&db)
            .await;

        let index = load_items_index(&db, &project.id).await.unwrap();
        assert_eq!(index.len(), 3);
        assert!(index.contains_key(&(1, ItemType::Issue)));
        assert!(index.contains_key(&(2, ItemType::Issue)));
        assert!(index.contains_key(&(3, ItemType::Issue)));
    }

    #[tokio::test]
    async fn load_items_index_scoped_to_project() {
        let (db, project) = setup().await;
        let connector = ConnectorFactory::default().create(&db).await;
        let other_project = ProjectFactory::default()
            .name("other-repo")
            .connector_id(&connector.id)
            .create(&db)
            .await;

        ItemFactory::new(&project.id, 1).create(&db).await;
        ItemFactory::new(&other_project.id, 2).create(&db).await;

        let index = load_items_index(&db, &project.id).await.unwrap();
        assert_eq!(index.len(), 1);
        assert!(index.contains_key(&(1, ItemType::Issue)));
    }

    #[tokio::test]
    async fn load_items_index_key_is_external_id_and_type() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .item_type(ItemType::Issue)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 1)
            .item_type(ItemType::PullRequest)
            .create(&db)
            .await;

        let index = load_items_index(&db, &project.id).await.unwrap();
        assert_eq!(index.len(), 2);
        assert!(index.contains_key(&(1, ItemType::Issue)));
        assert!(index.contains_key(&(1, ItemType::PullRequest)));
    }

    // -----------------------------------------------------------------------
    // upsert_items_batch
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn upsert_inserts_new_items() {
        let (db, project) = setup().await;
        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let new_items = vec![
            make_new_item(1, ItemType::Issue),
            make_new_item(2, ItemType::PullRequest),
        ];

        let saved = upsert_items_batch(&db, &project.id, &mut index, new_items)
            .await
            .unwrap();

        assert_eq!(saved.len(), 2);
        assert!(!saved[0].is_read);
        assert!(!saved[1].is_read);
        assert_eq!(saved[0].project_id, project.id);
    }

    #[tokio::test]
    async fn upsert_updates_existing_with_newer_timestamp() {
        let (db, project) = setup().await;
        let old_ts = dt("2024-01-01 00:00:00");
        let new_ts = dt("2024-02-01 00:00:00");

        ItemFactory::new(&project.id, 1)
            .updated_at(old_ts)
            .is_read(true)
            .title("Old Title")
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut new_item = make_new_item(1, ItemType::Issue);
        new_item.title = "New Title".to_string();
        new_item.updated_at = new_ts;

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![new_item])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].title, "New Title");
        // has_new_activity -> is_read set to false
        assert!(!saved[0].is_read);
    }

    #[tokio::test]
    async fn upsert_updates_existing_keeps_read_when_same_timestamp() {
        let (db, project) = setup().await;
        let ts = dt("2024-01-01 00:00:00");

        ItemFactory::new(&project.id, 1)
            .updated_at(ts)
            .is_read(true)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut new_item = make_new_item(1, ItemType::Issue);
        new_item.updated_at = ts; // same timestamp

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![new_item])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        // is_read should remain true (no new activity)
        assert!(saved[0].is_read);
    }

    #[tokio::test]
    async fn upsert_empty_batch() {
        let (db, project) = setup().await;
        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![])
            .await
            .unwrap();

        assert!(saved.is_empty());
    }

    #[tokio::test]
    async fn upsert_updates_index_preventing_duplicates() {
        let (db, project) = setup().await;
        let mut index = load_items_index(&db, &project.id).await.unwrap();
        assert!(index.is_empty());

        // First batch: insert
        let saved = upsert_items_batch(
            &db,
            &project.id,
            &mut index,
            vec![make_new_item(1, ItemType::Issue)],
        )
        .await
        .unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(index.len(), 1);

        // Second batch with same key: should update, not insert
        let mut updated = make_new_item(1, ItemType::Issue);
        updated.title = "Updated".to_string();
        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![updated])
            .await
            .unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].title, "Updated");

        // Total items in DB should be 1
        let all_items = item::Entity::find().all(&db).await.unwrap();
        assert_eq!(all_items.len(), 1);
    }

    #[rstest]
    #[case(ItemType::Issue, ItemType::Issue, true)] // same type -> update
    #[case(ItemType::Issue, ItemType::PullRequest, false)] // different type -> insert
    #[case(ItemType::PullRequest, ItemType::PullRequest, true)]
    #[case(ItemType::PullRequest, ItemType::Issue, false)]
    #[tokio::test]
    async fn upsert_matches_by_external_id_and_type(
        #[case] existing_type: ItemType,
        #[case] new_type: ItemType,
        #[case] is_update: bool,
    ) {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .item_type(existing_type)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();
        let initial_count = index.len();

        let new_item = make_new_item(1, new_type);
        upsert_items_batch(&db, &project.id, &mut index, vec![new_item])
            .await
            .unwrap();

        if is_update {
            assert_eq!(
                index.len(),
                initial_count,
                "should update existing, not add"
            );
        } else {
            assert_eq!(
                index.len(),
                initial_count + 1,
                "should insert new entry for different type"
            );
        }
    }

    // -----------------------------------------------------------------------
    // advance_sync_timestamp
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn advance_sync_timestamp_sets_current_time() {
        let (db, project) = setup().await;
        assert!(project.last_sync_at.is_none());

        let before = chrono::Utc::now().naive_utc();
        advance_sync_timestamp(&db, &project).await.unwrap();
        let after = chrono::Utc::now().naive_utc();

        let updated = project::Entity::find_by_id(&project.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let sync_at = updated.last_sync_at.expect("should be set");
        assert!(sync_at >= before && sync_at <= after);
    }

    #[tokio::test]
    async fn advance_sync_timestamp_clears_error() {
        let (db, _) = setup().await;
        let connector = ConnectorFactory::default().create(&db).await;
        let project = ProjectFactory::default()
            .name("err-test")
            .connector_id(&connector.id)
            .last_sync_error("previous error")
            .create(&db)
            .await;

        advance_sync_timestamp(&db, &project).await.unwrap();

        let updated = project::Entity::find_by_id(&project.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(updated.last_sync_error.is_none());
        assert!(updated.last_sync_at.is_some());
    }

    // -----------------------------------------------------------------------
    // mark_absent_items_closed
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn mark_absent_items_closed_closes_missing() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1).create(&db).await;
        ItemFactory::new(&project.id, 2).create(&db).await;
        ItemFactory::new(&project.id, 3).create(&db).await;

        // Only item 1 was fetched — items 2 and 3 should be closed
        mark_absent_items_closed(&db, &project.id, &[1], &ItemType::Issue)
            .await
            .unwrap();

        let items = item::Entity::find().all(&db).await.unwrap();
        let open: Vec<_> = items
            .iter()
            .filter(|i| {
                i.parse_type_data()
                    .map(|td| td.state() == Some(&ItemState::Open))
                    .unwrap_or(false)
            })
            .collect();
        let closed: Vec<_> = items
            .iter()
            .filter(|i| {
                i.parse_type_data()
                    .map(|td| td.state() == Some(&ItemState::Closed))
                    .unwrap_or(false)
            })
            .collect();

        assert_eq!(open.len(), 1);
        assert_eq!(open[0].parse_type_data().unwrap().external_id(), Some(1));
        assert_eq!(open[0].item_status, ItemStatus::Pending);
        assert_eq!(closed.len(), 2);
        assert!(closed.iter().all(|i| i.item_status == ItemStatus::Resolved));
    }

    #[tokio::test]
    async fn mark_absent_items_closed_respects_item_type() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .item_type(ItemType::Issue)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .item_type(ItemType::PullRequest)
            .create(&db)
            .await;

        // Close absent Issues — PR should be untouched
        mark_absent_items_closed(&db, &project.id, &[], &ItemType::Issue)
            .await
            .unwrap();

        let items = item::Entity::find().all(&db).await.unwrap();
        let issue = items
            .iter()
            .find(|i| i.parse_type_data().unwrap().external_id() == Some(1))
            .unwrap();
        let pr = items
            .iter()
            .find(|i| i.parse_type_data().unwrap().external_id() == Some(2))
            .unwrap();

        assert_eq!(
            issue.parse_type_data().unwrap().state(),
            Some(&ItemState::Closed)
        );
        assert_eq!(
            pr.parse_type_data().unwrap().state(),
            Some(&ItemState::Open),
        );
    }

    #[tokio::test]
    async fn mark_absent_items_closed_skips_deleted() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .item_status(ItemStatus::Deleted)
            .create(&db)
            .await;

        mark_absent_items_closed(&db, &project.id, &[], &ItemType::Issue)
            .await
            .unwrap();

        let item = item::Entity::find().one(&db).await.unwrap().unwrap();
        // Deleted items are excluded by the filter, so state stays Open and status stays Deleted
        assert_eq!(
            item.parse_type_data().unwrap().state(),
            Some(&ItemState::Open)
        );
        assert_eq!(item.item_status, ItemStatus::Deleted);
    }

    #[tokio::test]
    async fn mark_absent_items_closed_marks_already_closed_as_resolved() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .state(ItemState::Closed)
            .create(&db)
            .await;

        // Already-closed items that are absent should get item_status = resolved
        mark_absent_items_closed(&db, &project.id, &[], &ItemType::Issue)
            .await
            .unwrap();

        let item = item::Entity::find().one(&db).await.unwrap().unwrap();
        assert_eq!(
            item.parse_type_data().unwrap().state(),
            Some(&ItemState::Closed)
        );
        assert_eq!(item.item_status, ItemStatus::Resolved);
    }

    #[tokio::test]
    async fn mark_absent_items_closed_empty_fetched_ids_closes_all() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1).create(&db).await;
        ItemFactory::new(&project.id, 2).create(&db).await;

        mark_absent_items_closed(&db, &project.id, &[], &ItemType::Issue)
            .await
            .unwrap();

        let items = item::Entity::find().all(&db).await.unwrap();
        assert!(items.iter().all(|i| i
            .parse_type_data()
            .map(|td| td.state() == Some(&ItemState::Closed))
            .unwrap_or(false)));
        assert!(items.iter().all(|i| i.item_status == ItemStatus::Resolved));
    }

    // -----------------------------------------------------------------------
    // deactivate_closed_items
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn deactivate_closed_items_sets_resolved() {
        let (db, project) = setup().await;

        // Open item — should stay pending
        ItemFactory::new(&project.id, 1).create(&db).await;
        // Closed item with item_status=pending — should become resolved
        ItemFactory::new(&project.id, 2)
            .state(ItemState::Closed)
            .create(&db)
            .await;
        // Merged item with item_status=pending — should become resolved
        ItemFactory::new(&project.id, 3)
            .state(ItemState::Merged)
            .item_type(ItemType::PullRequest)
            .create(&db)
            .await;

        deactivate_closed_items(&db, &project.id).await.unwrap();

        let items = item::Entity::find().all(&db).await.unwrap();
        let open_item = items
            .iter()
            .find(|i| i.parse_type_data().unwrap().external_id() == Some(1))
            .unwrap();
        let closed_item = items
            .iter()
            .find(|i| i.parse_type_data().unwrap().external_id() == Some(2))
            .unwrap();
        let merged_item = items
            .iter()
            .find(|i| i.parse_type_data().unwrap().external_id() == Some(3))
            .unwrap();

        assert_eq!(open_item.item_status, ItemStatus::Pending);
        assert_eq!(closed_item.item_status, ItemStatus::Resolved);
        assert_eq!(merged_item.item_status, ItemStatus::Resolved);
    }

    // -----------------------------------------------------------------------
    // update_reconciliation_timestamp
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn update_reconciliation_timestamp_sets_time() {
        let (db, project) = setup().await;
        assert!(project.full_reconciliation_at.is_none());

        update_reconciliation_timestamp(&db, &project)
            .await
            .unwrap();

        let updated = project::Entity::find_by_id(&project.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(updated.full_reconciliation_at.is_some());
    }

    // -----------------------------------------------------------------------
    // set_sync_error
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn set_sync_error_records_message() {
        let (db, project) = setup().await;

        set_sync_error(&db, &project, "connection timeout")
            .await
            .unwrap();

        let updated = project::Entity::find_by_id(&project.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            updated.last_sync_error.as_deref(),
            Some("connection timeout")
        );
    }

    // -----------------------------------------------------------------------
    // upsert closed/merged → item_status = Resolved
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn upsert_marks_existing_closed_items_as_resolved() {
        let (db, project) = setup().await;

        // Create existing open items first
        ItemFactory::new(&project.id, 1)
            .item_type(ItemType::Issue)
            .create(&db)
            .await;
        ItemFactory::new(&project.id, 2)
            .item_type(ItemType::PullRequest)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut closed_item = make_new_item(1, ItemType::Issue);
        closed_item.state = ItemState::Closed;
        closed_item.updated_at = chrono::Utc::now().naive_utc();

        let mut merged_item = make_new_item(2, ItemType::PullRequest);
        merged_item.state = ItemState::Merged;
        merged_item.updated_at = chrono::Utc::now().naive_utc();

        let saved =
            upsert_items_batch(&db, &project.id, &mut index, vec![closed_item, merged_item])
                .await
                .unwrap();

        assert_eq!(saved.len(), 2);
        assert_eq!(
            saved[0].item_status,
            ItemStatus::Resolved,
            "closed item should be resolved"
        );
        assert_eq!(
            saved[1].item_status,
            ItemStatus::Resolved,
            "merged item should be resolved"
        );
    }

    // -----------------------------------------------------------------------
    // Skip new closed/merged items (incremental sync with state=all)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn upsert_skips_new_closed_items() {
        let (db, project) = setup().await;
        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut closed_issue = make_new_item(1, ItemType::Issue);
        closed_issue.state = ItemState::Closed;

        let mut merged_pr = make_new_item(2, ItemType::PullRequest);
        merged_pr.state = ItemState::Merged;

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![closed_issue, merged_pr])
            .await
            .unwrap();

        assert!(
            saved.is_empty(),
            "new closed/merged items should be skipped"
        );
        let all = item::Entity::find().all(&db).await.unwrap();
        assert_eq!(all.len(), 0, "nothing should be inserted in DB");
    }

    #[tokio::test]
    async fn upsert_skips_new_closed_but_saves_open() {
        let (db, project) = setup().await;
        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let open_issue = make_new_item(1, ItemType::Issue);

        let mut closed_issue = make_new_item(2, ItemType::Issue);
        closed_issue.state = ItemState::Closed;

        let mut merged_pr = make_new_item(3, ItemType::PullRequest);
        merged_pr.state = ItemState::Merged;

        let saved = upsert_items_batch(
            &db,
            &project.id,
            &mut index,
            vec![open_issue, closed_issue, merged_pr],
        )
        .await
        .unwrap();

        assert_eq!(saved.len(), 1, "only the open item should be saved");
        let td = saved[0].parse_type_data().unwrap();
        assert_eq!(td.external_id(), Some(1));
        assert_eq!(td.state(), Some(&ItemState::Open));
        assert_eq!(index.len(), 1);
    }

    // -----------------------------------------------------------------------
    // State transitions (open -> closed/merged)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn upsert_open_to_closed_sets_resolved() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .item_type(ItemType::Issue)
            .state(ItemState::Open)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut closed = make_new_item(1, ItemType::Issue);
        closed.state = ItemState::Closed;
        closed.updated_at = chrono::Utc::now().naive_utc();

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![closed])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].item_status, ItemStatus::Resolved);
        let td = saved[0].parse_type_data().unwrap();
        assert_eq!(td.state(), Some(&ItemState::Closed));
    }

    #[tokio::test]
    async fn upsert_open_to_merged_sets_resolved() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .item_type(ItemType::PullRequest)
            .state(ItemState::Open)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut merged = make_new_item(1, ItemType::PullRequest);
        merged.state = ItemState::Merged;
        merged.updated_at = chrono::Utc::now().naive_utc();

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![merged])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].item_status, ItemStatus::Resolved);
        let td = saved[0].parse_type_data().unwrap();
        assert_eq!(td.state(), Some(&ItemState::Merged));
    }

    // -----------------------------------------------------------------------
    // Mixed scenarios
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn upsert_batch_closes_existing_and_skips_new_closed() {
        let (db, project) = setup().await;

        // Existing open issue #1
        ItemFactory::new(&project.id, 1)
            .item_type(ItemType::Issue)
            .state(ItemState::Open)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        // Issue #1 now closed (update), Issue #2 new and closed (skip)
        let mut closed_existing = make_new_item(1, ItemType::Issue);
        closed_existing.state = ItemState::Closed;
        closed_existing.updated_at = chrono::Utc::now().naive_utc();

        let mut closed_new = make_new_item(2, ItemType::Issue);
        closed_new.state = ItemState::Closed;

        let saved = upsert_items_batch(
            &db,
            &project.id,
            &mut index,
            vec![closed_existing, closed_new],
        )
        .await
        .unwrap();

        // Only existing item #1 should be updated
        assert_eq!(saved.len(), 1);
        let td = saved[0].parse_type_data().unwrap();
        assert_eq!(td.external_id(), Some(1));
        assert_eq!(saved[0].item_status, ItemStatus::Resolved);
        assert_eq!(td.state(), Some(&ItemState::Closed));

        // DB should have only 1 item (the updated one)
        let all = item::Entity::find().all(&db).await.unwrap();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn upsert_all_new_items_closed_returns_empty() {
        let (db, project) = setup().await;
        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut closed_issue = make_new_item(1, ItemType::Issue);
        closed_issue.state = ItemState::Closed;

        let mut merged_pr = make_new_item(2, ItemType::PullRequest);
        merged_pr.state = ItemState::Merged;

        let mut closed_discussion = make_new_item(3, ItemType::Discussion);
        closed_discussion.state = ItemState::Closed;

        let saved = upsert_items_batch(
            &db,
            &project.id,
            &mut index,
            vec![closed_issue, merged_pr, closed_discussion],
        )
        .await
        .unwrap();

        assert!(saved.is_empty());
        let all = item::Entity::find().all(&db).await.unwrap();
        assert_eq!(all.len(), 0);
    }

    #[tokio::test]
    async fn upsert_reopened_item_becomes_pending() {
        let (db, project) = setup().await;

        // Create an existing closed+resolved item
        ItemFactory::new(&project.id, 1)
            .state(ItemState::Closed)
            .item_status(ItemStatus::Resolved)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();
        assert_eq!(index.len(), 1);

        // Re-sync the item as open
        let mut reopened = make_new_item(1, ItemType::Issue);
        reopened.state = ItemState::Open;
        reopened.updated_at = chrono::Utc::now().naive_utc();

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![reopened])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        let td = saved[0].parse_type_data().unwrap();
        assert_eq!(td.state(), Some(&ItemState::Open));
        assert_eq!(saved[0].item_status, ItemStatus::Pending);
    }

    // -----------------------------------------------------------------------
    // item_status flow tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn upsert_dismissed_item_with_new_activity_undismisses() {
        let (db, project) = setup().await;
        let old_ts = dt("2024-01-01 00:00:00");

        ItemFactory::new(&project.id, 1)
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(old_ts)
            .updated_at(old_ts)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut new_item = make_new_item(1, ItemType::Issue);
        new_item.updated_at = chrono::Utc::now().naive_utc(); // newer → has_new_activity

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![new_item])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].item_status, ItemStatus::Pending);
        assert!(!saved[0].is_read);
    }

    #[tokio::test]
    async fn upsert_dismissed_item_closed_becomes_resolved() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(now())
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut closed = make_new_item(1, ItemType::Issue);
        closed.state = ItemState::Closed;
        closed.updated_at = chrono::Utc::now().naive_utc();

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![closed])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(
            saved[0].item_status,
            ItemStatus::Resolved,
            "provider wins over dismiss"
        );
    }

    #[tokio::test]
    async fn upsert_dismissed_item_no_new_activity_stays_dismissed() {
        let (db, project) = setup().await;
        let ts = dt("2024-06-01 12:00:00");

        ItemFactory::new(&project.id, 1)
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(ts)
            .updated_at(ts)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut same = make_new_item(1, ItemType::Issue);
        same.updated_at = ts; // same timestamp → no new activity

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![same])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].item_status, ItemStatus::Dismissed);
    }

    #[tokio::test]
    async fn upsert_deleted_item_stays_deleted() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .item_status(ItemStatus::Deleted)
            .create(&db)
            .await;

        let mut index = load_items_index(&db, &project.id).await.unwrap();

        let mut open = make_new_item(1, ItemType::Issue);
        open.state = ItemState::Open;
        open.updated_at = chrono::Utc::now().naive_utc();

        let saved = upsert_items_batch(&db, &project.id, &mut index, vec![open])
            .await
            .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(
            saved[0].item_status,
            ItemStatus::Deleted,
            "user delete is permanent"
        );
    }

    #[tokio::test]
    async fn mark_absent_resolves_dismissed_items() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .item_status(ItemStatus::Dismissed)
            .dismissed_at(now())
            .create(&db)
            .await;

        mark_absent_items_closed(&db, &project.id, &[], &ItemType::Issue)
            .await
            .unwrap();

        let item = item::Entity::find().one(&db).await.unwrap().unwrap();
        assert_eq!(item.item_status, ItemStatus::Resolved);
    }

    #[tokio::test]
    async fn deactivate_closed_items_skips_deleted() {
        let (db, project) = setup().await;

        ItemFactory::new(&project.id, 1)
            .state(ItemState::Closed)
            .item_status(ItemStatus::Deleted)
            .create(&db)
            .await;

        deactivate_closed_items(&db, &project.id).await.unwrap();

        let item = item::Entity::find().one(&db).await.unwrap().unwrap();
        assert_eq!(
            item.item_status,
            ItemStatus::Deleted,
            "deleted items should not become resolved"
        );
    }
}
