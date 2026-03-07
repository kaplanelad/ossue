use chrono::NaiveDateTime;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, Set};
use uuid::Uuid;

use crate::enums::{
    ActionType, DraftIssueStatus, ItemState, ItemStatus, ItemType, ItemTypeData, NoteData,
    NoteType, Platform, PrItemData, ProviderItemData, ProviderMode,
};
use crate::migration::Migrator;
use crate::models::{
    analysis_history, chat_message, connector, item, project, project_note, project_settings,
};
use crate::sync::NewItem;

use sea_orm_migration::MigratorTrait;

/// Create a fresh in-memory SQLite database with all migrations applied.
pub async fn setup_test_db() -> DatabaseConnection {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared("PRAGMA foreign_keys=ON")
        .await
        .unwrap();
    Migrator::up(&db, None).await.unwrap();
    db
}

/// Parse a datetime string like "2024-01-15 10:30:00".
pub fn dt(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
}

/// Current UTC time as NaiveDateTime.
pub fn now() -> NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

/// Build a [`NewItem`] for use in sync tests.
pub fn make_new_item(external_id: i32, item_type: ItemType) -> NewItem {
    let ts = now();
    NewItem {
        external_id,
        item_type,
        title: format!("Item #{external_id}"),
        body: format!("Body of item #{external_id}"),
        state: ItemState::Open,
        author: "test-author".to_string(),
        url: format!("https://example.com/items/{external_id}"),
        comments_count: 0,
        pr_branch: None,
        labels: Vec::new(),
        created_at: ts,
        updated_at: ts,
    }
}

// ---------------------------------------------------------------------------
// Factory: Connector
// ---------------------------------------------------------------------------

pub struct ConnectorFactory {
    name: String,
    platform: String,
    token: String,
    base_url: Option<String>,
}

impl Default for ConnectorFactory {
    fn default() -> Self {
        Self {
            name: "test-connector".to_string(),
            platform: "github".to_string(),
            token: "ghp_test_token".to_string(),
            base_url: None,
        }
    }
}

impl ConnectorFactory {
    pub fn platform(mut self, p: &str) -> Self {
        self.platform = p.to_string();
        self
    }

    pub fn base_url(mut self, u: &str) -> Self {
        self.base_url = Some(u.to_string());
        self
    }

    pub async fn create(self, db: &DatabaseConnection) -> connector::Model {
        let id = Uuid::new_v4().to_string();
        let ts = now();
        let active = connector::ActiveModel {
            id: Set(id),
            name: Set(self.name),
            platform: Set(self.platform),
            token: Set(self.token),
            base_url: Set(self.base_url),
            created_at: Set(ts),
            updated_at: Set(ts),
        };
        active.insert(db).await.unwrap()
    }
}

// ---------------------------------------------------------------------------
// Factory: Project
// ---------------------------------------------------------------------------

pub struct ProjectFactory {
    name: String,
    owner: String,
    platform: Platform,
    connector_id: Option<String>,
    sync_enabled: bool,
    last_sync_at: Option<NaiveDateTime>,
    last_sync_error: Option<String>,
}

impl Default for ProjectFactory {
    fn default() -> Self {
        Self {
            name: "test-repo".to_string(),
            owner: "test-owner".to_string(),
            platform: Platform::GitHub,
            connector_id: None,
            sync_enabled: true,
            last_sync_at: None,
            last_sync_error: None,
        }
    }
}

impl ProjectFactory {
    pub fn name(mut self, n: &str) -> Self {
        self.name = n.to_string();
        self
    }

    pub fn connector_id(mut self, id: &str) -> Self {
        self.connector_id = Some(id.to_string());
        self
    }

    pub fn last_sync_at(mut self, ts: NaiveDateTime) -> Self {
        self.last_sync_at = Some(ts);
        self
    }

    pub fn last_sync_error(mut self, msg: &str) -> Self {
        self.last_sync_error = Some(msg.to_string());
        self
    }

    pub async fn create(self, db: &DatabaseConnection) -> project::Model {
        let id = Uuid::new_v4().to_string();
        let ts = now();
        let active = project::ActiveModel {
            id: Set(id),
            name: Set(self.name),
            owner: Set(self.owner),
            platform: Set(self.platform),
            url: Set("https://github.com/test-owner/test-repo".to_string()),
            clone_path: Set(None),
            default_branch: Set(Some("main".to_string())),
            api_token: Set(None),
            connector_id: Set(self.connector_id),
            external_project_id: Set(None),
            sync_enabled: Set(self.sync_enabled),
            last_sync_at: Set(self.last_sync_at),
            last_sync_error: Set(self.last_sync_error),
            full_reconciliation_at: Set(None),
            created_at: Set(ts),
            updated_at: Set(ts),
        };
        active.insert(db).await.unwrap()
    }
}

// ---------------------------------------------------------------------------
// Factory: Item
// ---------------------------------------------------------------------------

pub struct ItemFactory {
    project_id: String,
    external_id: i32,
    item_type: ItemType,
    state: ItemState,
    is_deleted: bool,
    item_status: ItemStatus,
    is_read: bool,
    is_starred: bool,
    dismissed_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
    title: Option<String>,
    body: Option<String>,
    author: Option<String>,
    labels: Option<Vec<String>>,
    pr_branch: Option<String>,
    draft_status: Option<DraftIssueStatus>,
}

impl ItemFactory {
    pub fn new(project_id: &str, external_id: i32) -> Self {
        Self {
            project_id: project_id.to_string(),
            external_id,
            item_type: ItemType::Issue,
            state: ItemState::Open,
            is_deleted: false,
            item_status: ItemStatus::Pending,
            is_read: false,
            is_starred: false,
            dismissed_at: None,
            updated_at: None,
            title: None,
            body: None,
            author: None,
            labels: None,
            pr_branch: None,
            draft_status: None,
        }
    }

    pub fn item_type(mut self, t: ItemType) -> Self {
        self.item_type = t;
        self
    }

    pub fn state(mut self, s: ItemState) -> Self {
        self.state = s;
        self
    }

    pub fn is_deleted(mut self, d: bool) -> Self {
        self.is_deleted = d;
        self
    }

    pub fn item_status(mut self, s: ItemStatus) -> Self {
        self.item_status = s;
        self
    }

    pub fn is_read(mut self, r: bool) -> Self {
        self.is_read = r;
        self
    }

    pub fn is_starred(mut self, s: bool) -> Self {
        self.is_starred = s;
        self
    }

    pub fn dismissed_at(mut self, ts: NaiveDateTime) -> Self {
        self.dismissed_at = Some(ts);
        self
    }

    pub fn updated_at(mut self, ts: NaiveDateTime) -> Self {
        self.updated_at = Some(ts);
        self
    }

    pub fn title(mut self, t: &str) -> Self {
        self.title = Some(t.to_string());
        self
    }

    pub fn body(mut self, b: &str) -> Self {
        self.body = Some(b.to_string());
        self
    }

    pub fn author(mut self, a: &str) -> Self {
        self.author = Some(a.to_string());
        self
    }

    pub fn labels(mut self, l: Vec<&str>) -> Self {
        self.labels = Some(l.into_iter().map(String::from).collect());
        self
    }

    pub fn pr_branch(mut self, b: &str) -> Self {
        self.pr_branch = Some(b.to_string());
        self
    }

    pub fn draft_status(mut self, s: DraftIssueStatus) -> Self {
        self.draft_status = Some(s);
        self
    }

    pub async fn create(self, db: &DatabaseConnection) -> item::Model {
        let id = Uuid::new_v4().to_string();
        let ts = self.updated_at.unwrap_or_else(now);

        let provider = ProviderItemData {
            external_id: self.external_id,
            state: self.state.clone(),
            author: self.author.unwrap_or_else(|| "test-author".to_string()),
            url: format!("https://example.com/items/{}", self.external_id),
            comments_count: 0,
            fetched_at: ts.to_string(),
            labels: self.labels.unwrap_or_default(),
        };

        let type_data = match self.item_type {
            ItemType::PullRequest => serde_json::to_string(&ItemTypeData::Pr(PrItemData {
                provider,
                pr_branch: self.pr_branch,
                pr_diff: None,
            }))
            .unwrap(),
            ItemType::Discussion => {
                serde_json::to_string(&ItemTypeData::Discussion(provider)).unwrap()
            }
            ItemType::Note => serde_json::to_string(&ItemTypeData::Note(NoteData {
                raw_content: "test note".to_string(),
                draft_status: self.draft_status.clone().unwrap_or(DraftIssueStatus::Draft),
                labels: None,
                priority: None,
                area: None,
                provider_issue_number: None,
                provider_issue_url: None,
            }))
            .unwrap(),
            _ => serde_json::to_string(&ItemTypeData::Issue(provider)).unwrap(),
        };

        let active = item::ActiveModel {
            id: Set(id),
            project_id: Set(self.project_id),
            item_type: Set(self.item_type),
            title: Set(self
                .title
                .unwrap_or_else(|| format!("Item #{}", self.external_id))),
            body: Set(self.body.unwrap_or_else(|| "test body".to_string())),
            type_data: Set(type_data),
            is_read: Set(self.is_read),
            is_starred: Set(self.is_starred),
            is_deleted: Set(self.is_deleted),
            item_status: Set(self.item_status),
            dismissed_at: Set(self.dismissed_at),
            created_at: Set(ts),
            updated_at: Set(ts),
        };
        active.insert(db).await.unwrap()
    }
}

// ---------------------------------------------------------------------------
// Factory: ChatMessage
// ---------------------------------------------------------------------------

pub struct ChatMessageFactory {
    item_id: String,
    role: String,
    content: String,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
}

impl ChatMessageFactory {
    pub fn new(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            role: "assistant".to_string(),
            content: "test message".to_string(),
            input_tokens: None,
            output_tokens: None,
        }
    }

    pub fn role(mut self, r: &str) -> Self {
        self.role = r.to_string();
        self
    }

    pub fn content(mut self, c: &str) -> Self {
        self.content = c.to_string();
        self
    }

    pub async fn create(self, db: &DatabaseConnection) -> chat_message::Model {
        let id = Uuid::new_v4().to_string();
        let ts = now();
        let active = chat_message::ActiveModel {
            id: Set(id),
            item_id: Set(self.item_id),
            role: Set(self.role),
            content: Set(self.content),
            created_at: Set(ts),
            input_tokens: Set(self.input_tokens),
            output_tokens: Set(self.output_tokens),
            model: Set(None),
        };
        active.insert(db).await.unwrap()
    }
}

// ---------------------------------------------------------------------------
// Factory: ProjectNote
// ---------------------------------------------------------------------------

pub struct ProjectNoteFactory {
    project_id: String,
    note_type: NoteType,
    content: String,
}

impl ProjectNoteFactory {
    pub fn new(project_id: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            note_type: NoteType::Manual,
            content: "test note".to_string(),
        }
    }

    pub fn note_type(mut self, t: NoteType) -> Self {
        self.note_type = t;
        self
    }

    pub fn content(mut self, c: &str) -> Self {
        self.content = c.to_string();
        self
    }

    pub async fn create(self, db: &DatabaseConnection) -> project_note::Model {
        let id = Uuid::new_v4().to_string();
        let ts = now();
        let active = project_note::ActiveModel {
            id: Set(id),
            project_id: Set(self.project_id),
            note_type: Set(self.note_type),
            content: Set(self.content),
            created_at: Set(ts),
            updated_at: Set(ts),
        };
        active.insert(db).await.unwrap()
    }
}

// ---------------------------------------------------------------------------
// Factory: AnalysisHistory
// ---------------------------------------------------------------------------

pub struct AnalysisHistoryFactory {
    item_id: String,
    action_type: ActionType,
    provider_mode: ProviderMode,
}

impl AnalysisHistoryFactory {
    pub fn new(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            action_type: ActionType::Review,
            provider_mode: ProviderMode::Api,
        }
    }

    pub fn action_type(mut self, a: ActionType) -> Self {
        self.action_type = a;
        self
    }

    pub fn provider_mode(mut self, p: ProviderMode) -> Self {
        self.provider_mode = p;
        self
    }

    pub async fn create(self, db: &DatabaseConnection) -> analysis_history::Model {
        let id = Uuid::new_v4().to_string();
        let ts = now();
        let active = analysis_history::ActiveModel {
            id: Set(id),
            item_id: Set(self.item_id),
            action_type: Set(self.action_type),
            provider_mode: Set(self.provider_mode),
            prompt_hash: Set("testhash".to_string()),
            created_at: Set(ts),
        };
        active.insert(db).await.unwrap()
    }
}

// ---------------------------------------------------------------------------
// Factory: ProjectSettings
// ---------------------------------------------------------------------------

pub struct ProjectSettingsFactory {
    project_id: String,
    key: String,
    value: String,
}

impl ProjectSettingsFactory {
    pub fn new(project_id: &str, key: &str, value: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    pub async fn create(self, db: &DatabaseConnection) -> project_settings::Model {
        let active = project_settings::ActiveModel {
            project_id: Set(self.project_id),
            key: Set(self.key),
            value: Set(self.value),
        };
        active.insert(db).await.unwrap()
    }
}
