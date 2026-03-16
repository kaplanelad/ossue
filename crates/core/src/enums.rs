use std::fmt;

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ItemStatus {
    #[sea_orm(string_value = "pending")]
    #[serde(rename = "pending")]
    Pending,
    #[sea_orm(string_value = "resolved")]
    #[serde(rename = "resolved")]
    Resolved,
    #[sea_orm(string_value = "dismissed")]
    #[serde(rename = "dismissed")]
    Dismissed,
    #[sea_orm(string_value = "deleted")]
    #[serde(rename = "deleted")]
    Deleted,
}

impl fmt::Display for ItemStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Resolved => write!(f, "resolved"),
            Self::Dismissed => write!(f, "dismissed"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ItemType {
    #[sea_orm(string_value = "issue")]
    #[serde(rename = "issue")]
    Issue,
    #[sea_orm(string_value = "pr")]
    #[serde(rename = "pr")]
    PullRequest,
    #[sea_orm(string_value = "discussion")]
    #[serde(rename = "discussion")]
    Discussion,
    #[sea_orm(string_value = "note")]
    #[serde(rename = "note")]
    Note,
}

impl fmt::Display for ItemType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Issue => write!(f, "issue"),
            Self::PullRequest => write!(f, "pr"),
            Self::Discussion => write!(f, "discussion"),
            Self::Note => write!(f, "note"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemState {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "merged")]
    Merged,
}

impl fmt::Display for ItemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Closed => write!(f, "closed"),
            Self::Merged => write!(f, "merged"),
        }
    }
}

impl ItemState {
    /// Parse from GitHub API state string (REST: "open"/"closed", GraphQL: "OPEN"/"CLOSED"/"MERGED")
    pub fn from_github_state(state: &str, merged: Option<bool>) -> Self {
        if merged.unwrap_or(false) {
            return Self::Merged;
        }
        match state {
            "open" | "OPEN" => Self::Open,
            "closed" | "CLOSED" => Self::Closed,
            "merged" | "MERGED" => Self::Merged,
            _ => Self::Open,
        }
    }

    /// Parse from GitLab API state string ("opened", "closed", "merged")
    pub fn from_gitlab_state(state: &str) -> Self {
        match state {
            "opened" => Self::Open,
            "closed" => Self::Closed,
            "merged" => Self::Merged,
            _ => Self::Open,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum Platform {
    #[sea_orm(string_value = "github")]
    #[serde(rename = "github")]
    GitHub,
    #[sea_orm(string_value = "gitlab")]
    #[serde(rename = "gitlab")]
    GitLab,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitHub => write!(f, "github"),
            Self::GitLab => write!(f, "gitlab"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum NoteType {
    #[sea_orm(string_value = "auto")]
    #[serde(rename = "auto")]
    Auto,
    #[sea_orm(string_value = "manual")]
    #[serde(rename = "manual")]
    Manual,
}

impl fmt::Display for NoteType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ActionType {
    #[sea_orm(string_value = "analyze")]
    #[serde(rename = "analyze")]
    Analyze,
    #[sea_orm(string_value = "draft_response")]
    #[serde(rename = "draft_response")]
    DraftResponse,
}

impl fmt::Display for ActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analyze => write!(f, "analyze"),
            Self::DraftResponse => write!(f, "draft_response"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ProviderMode {
    #[sea_orm(string_value = "api")]
    #[serde(rename = "api")]
    Api,
    #[sea_orm(string_value = "cli")]
    #[serde(rename = "cli")]
    Cli,
}

impl fmt::Display for ProviderMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api => write!(f, "api"),
            Self::Cli => write!(f, "cli"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiMode {
    #[serde(rename = "api")]
    Api,
    #[serde(rename = "cli")]
    Cli,
}

impl AiMode {
    pub fn is_api(&self) -> bool {
        matches!(self, Self::Api)
    }
}

impl fmt::Display for AiMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api => write!(f, "api"),
            Self::Cli => write!(f, "cli"),
        }
    }
}

impl AiMode {
    pub fn from_setting(s: &str) -> Self {
        match s {
            "api" => Self::Api,
            _ => Self::Cli,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum MessageRole {
    #[sea_orm(string_value = "user")]
    #[serde(rename = "user")]
    User,
    #[sea_orm(string_value = "assistant")]
    #[serde(rename = "assistant")]
    Assistant,
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OAuthStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "slow_down")]
    SlowDown,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "denied")]
    Denied,
    #[serde(rename = "error")]
    Error,
}

impl fmt::Display for OAuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Success => write!(f, "success"),
            Self::SlowDown => write!(f, "slow_down"),
            Self::Expired => write!(f, "expired"),
            Self::Denied => write!(f, "denied"),
            Self::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DraftIssueStatus {
    #[serde(rename = "draft")]
    Draft,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "submitted")]
    Submitted,
}

impl fmt::Display for DraftIssueStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Ready => write!(f, "ready"),
            Self::Submitted => write!(f, "submitted"),
        }
    }
}

/// Shared fields for provider items (issue, PR, discussion).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderItemData {
    pub external_id: i32,
    pub state: ItemState,
    pub author: String,
    pub url: String,
    pub comments_count: i32,
    pub fetched_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

/// Additional fields for pull requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrItemData {
    #[serde(flatten)]
    pub provider: ProviderItemData,
    pub pr_branch: Option<String>,
    pub pr_diff: Option<String>,
}

/// Fields for notes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteData {
    pub raw_content: String,
    pub draft_status: DraftIssueStatus,
    pub labels: Option<Vec<String>>,
    pub priority: Option<String>,
    pub area: Option<String>,
    pub provider_issue_number: Option<i32>,
    pub provider_issue_url: Option<String>,
}

/// Tagged union stored in the `type_data` column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ItemTypeData {
    #[serde(rename = "issue")]
    Issue(ProviderItemData),
    #[serde(rename = "pr")]
    Pr(PrItemData),
    #[serde(rename = "discussion")]
    Discussion(ProviderItemData),
    #[serde(rename = "note")]
    Note(NoteData),
}

impl ItemTypeData {
    pub fn external_id(&self) -> Option<i32> {
        match self {
            Self::Issue(d) | Self::Discussion(d) => Some(d.external_id),
            Self::Pr(d) => Some(d.provider.external_id),
            Self::Note(_) => None,
        }
    }

    pub fn state(&self) -> Option<&ItemState> {
        match self {
            Self::Issue(d) | Self::Discussion(d) => Some(&d.state),
            Self::Pr(d) => Some(&d.provider.state),
            Self::Note(_) => None,
        }
    }

    pub fn author(&self) -> Option<&str> {
        match self {
            Self::Issue(d) | Self::Discussion(d) => Some(&d.author),
            Self::Pr(d) => Some(&d.provider.author),
            Self::Note(_) => None,
        }
    }

    pub fn url(&self) -> Option<&str> {
        match self {
            Self::Issue(d) | Self::Discussion(d) => Some(&d.url),
            Self::Pr(d) => Some(&d.provider.url),
            Self::Note(_) => None,
        }
    }

    pub fn draft_status(&self) -> Option<&DraftIssueStatus> {
        match self {
            Self::Note(d) => Some(&d.draft_status),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // -----------------------------------------------------------------------
    // Display impls
    // -----------------------------------------------------------------------

    #[rstest]
    #[case(ItemStatus::Pending, "pending")]
    #[case(ItemStatus::Resolved, "resolved")]
    #[case(ItemStatus::Dismissed, "dismissed")]
    #[case(ItemStatus::Deleted, "deleted")]
    fn item_status_display(#[case] variant: ItemStatus, #[case] expected: &str) {
        assert_eq!(variant.to_string(), expected);
    }

    #[rstest]
    #[case(ItemStatus::Pending)]
    #[case(ItemStatus::Resolved)]
    #[case(ItemStatus::Dismissed)]
    #[case(ItemStatus::Deleted)]
    fn item_status_serde_roundtrip(#[case] variant: ItemStatus) {
        let json = serde_json::to_string(&variant).unwrap();
        let back: ItemStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }

    #[rstest]
    #[case(ItemType::Issue, "issue")]
    #[case(ItemType::PullRequest, "pr")]
    #[case(ItemType::Discussion, "discussion")]
    #[case(ItemType::Note, "note")]
    fn item_type_display(#[case] variant: ItemType, #[case] expected: &str) {
        assert_eq!(variant.to_string(), expected);
    }

    #[rstest]
    #[case(ItemState::Open, "open")]
    #[case(ItemState::Closed, "closed")]
    #[case(ItemState::Merged, "merged")]
    fn item_state_display(#[case] variant: ItemState, #[case] expected: &str) {
        assert_eq!(variant.to_string(), expected);
    }

    #[rstest]
    #[case(Platform::GitHub, "github")]
    #[case(Platform::GitLab, "gitlab")]
    fn platform_display(#[case] variant: Platform, #[case] expected: &str) {
        assert_eq!(variant.to_string(), expected);
    }

    #[rstest]
    #[case(NoteType::Auto, "auto")]
    #[case(NoteType::Manual, "manual")]
    fn note_type_display(#[case] variant: NoteType, #[case] expected: &str) {
        assert_eq!(variant.to_string(), expected);
    }

    #[rstest]
    #[case(ActionType::Analyze, "analyze")]
    #[case(ActionType::DraftResponse, "draft_response")]
    fn action_type_display(#[case] variant: ActionType, #[case] expected: &str) {
        assert_eq!(variant.to_string(), expected);
    }

    #[rstest]
    #[case(ProviderMode::Api, "api")]
    #[case(ProviderMode::Cli, "cli")]
    fn provider_mode_display(#[case] variant: ProviderMode, #[case] expected: &str) {
        assert_eq!(variant.to_string(), expected);
    }

    #[rstest]
    #[case(DraftIssueStatus::Draft, "draft")]
    #[case(DraftIssueStatus::Ready, "ready")]
    #[case(DraftIssueStatus::Submitted, "submitted")]
    fn draft_issue_status_display(#[case] variant: DraftIssueStatus, #[case] expected: &str) {
        assert_eq!(variant.to_string(), expected);
    }

    // -----------------------------------------------------------------------
    // ItemState::from_github_state
    // -----------------------------------------------------------------------

    #[rstest]
    #[case("open", None, ItemState::Open)]
    #[case("open", Some(false), ItemState::Open)]
    #[case("closed", None, ItemState::Closed)]
    #[case("closed", Some(false), ItemState::Closed)]
    #[case("closed", Some(true), ItemState::Merged)]
    #[case("OPEN", None, ItemState::Open)]
    #[case("CLOSED", None, ItemState::Closed)]
    #[case("MERGED", None, ItemState::Merged)]
    #[case("unknown", None, ItemState::Open)]
    fn from_github_state(
        #[case] state: &str,
        #[case] merged: Option<bool>,
        #[case] expected: ItemState,
    ) {
        assert_eq!(ItemState::from_github_state(state, merged), expected);
    }

    // -----------------------------------------------------------------------
    // ItemState::from_gitlab_state
    // -----------------------------------------------------------------------

    #[rstest]
    #[case("opened", ItemState::Open)]
    #[case("closed", ItemState::Closed)]
    #[case("merged", ItemState::Merged)]
    #[case("unknown", ItemState::Open)]
    fn from_gitlab_state(#[case] state: &str, #[case] expected: ItemState) {
        assert_eq!(ItemState::from_gitlab_state(state), expected);
    }

    // -----------------------------------------------------------------------
    // Serde roundtrips
    // -----------------------------------------------------------------------

    #[rstest]
    #[case(ItemType::Issue)]
    #[case(ItemType::PullRequest)]
    #[case(ItemType::Discussion)]
    #[case(ItemType::Note)]
    fn item_type_serde_roundtrip(#[case] variant: ItemType) {
        let json = serde_json::to_string(&variant).unwrap();
        let back: ItemType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }

    #[rstest]
    #[case(ItemState::Open)]
    #[case(ItemState::Closed)]
    #[case(ItemState::Merged)]
    fn item_state_serde_roundtrip(#[case] variant: ItemState) {
        let json = serde_json::to_string(&variant).unwrap();
        let back: ItemState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }

    #[rstest]
    #[case(Platform::GitHub)]
    #[case(Platform::GitLab)]
    fn platform_serde_roundtrip(#[case] variant: Platform) {
        let json = serde_json::to_string(&variant).unwrap();
        let back: Platform = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}
