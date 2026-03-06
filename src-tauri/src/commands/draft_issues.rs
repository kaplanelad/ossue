use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use super::error::CommandError;
use crate::AppState;
use ossue_core::enums::{DraftIssueStatus, ItemType, ItemTypeData, NoteData};
use ossue_core::models::item;
use ossue_core::services::issue_creator::{CreateIssueRequest, CreateIssueResponse, IssueCreator};

#[derive(Debug, Serialize, Deserialize)]
pub struct DraftIssueResponse {
    pub id: String,
    pub project_id: String,
    pub status: String,
    pub raw_content: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
    pub priority: Option<String>,
    pub area: Option<String>,
    pub provider_issue_number: Option<i32>,
    pub provider_issue_url: Option<String>,
    pub is_starred: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<item::Model> for DraftIssueResponse {
    type Error = String;
    fn try_from(m: item::Model) -> Result<Self, String> {
        let note_data = match m.parse_type_data().map_err(|e| e.to_string())? {
            ItemTypeData::Note(n) => n,
            _ => return Err("Not a note item".to_string()),
        };
        Ok(Self {
            id: m.id,
            project_id: m.project_id,
            status: note_data.draft_status.to_string(),
            raw_content: note_data.raw_content,
            title: if m.title.is_empty() {
                None
            } else {
                Some(m.title)
            },
            body: if m.body.is_empty() {
                None
            } else {
                Some(m.body)
            },
            labels: note_data.labels,
            priority: note_data.priority,
            area: note_data.area,
            provider_issue_number: note_data.provider_issue_number,
            provider_issue_url: note_data.provider_issue_url,
            is_starred: m.is_starred,
            created_at: m.created_at.to_string(),
            updated_at: m.updated_at.to_string(),
        })
    }
}

#[tauri::command]
pub async fn list_draft_issues(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> Result<Vec<DraftIssueResponse>, CommandError> {
    tracing::debug!(project_id = ?project_id, "Listing draft issues");
    let db = state.get_db().await?;

    let mut query = item::Entity::find().filter(item::Column::ItemType.eq("note"));

    if let Some(ref pid) = project_id {
        query = query.filter(item::Column::ProjectId.eq(pid));
    }

    let drafts = query
        .order_by_desc(item::Column::CreatedAt)
        .all(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to query draft issues");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    // Post-filter: exclude submitted notes and convert via TryFrom
    let results: Vec<DraftIssueResponse> = drafts
        .into_iter()
        .filter_map(|d| {
            let resp = DraftIssueResponse::try_from(d).ok()?;
            if resp.status == "submitted" {
                None
            } else {
                Some(resp)
            }
        })
        .collect();

    tracing::debug!(count = results.len(), "Retrieved draft issues");
    Ok(results)
}

#[tauri::command]
pub async fn create_draft_issue(
    state: State<'_, AppState>,
    project_id: String,
    raw_content: String,
) -> Result<DraftIssueResponse, CommandError> {
    if raw_content.trim().is_empty() {
        return Err(CommandError::Internal {
            message: "Content cannot be empty".to_string(),
        });
    }

    tracing::info!(project_id = %project_id, "Creating draft issue");
    let db = state.get_db().await?;

    let now = chrono::Utc::now().naive_utc();

    let type_data = serde_json::to_string(&ItemTypeData::Note(NoteData {
        raw_content: raw_content.clone(),
        draft_status: DraftIssueStatus::Draft,
        labels: None,
        priority: None,
        area: None,
        provider_issue_number: None,
        provider_issue_url: None,
    }))
    .map_err(|e| CommandError::Internal {
        message: e.to_string(),
    })?;

    let model = item::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(project_id.clone()),
        item_type: Set(ItemType::Note),
        title: Set(String::new()),
        body: Set(String::new()),
        type_data: Set(type_data),
        is_read: Set(true),
        is_starred: Set(false),
        is_deleted: Set(false),
        item_status: Set(ossue_core::enums::ItemStatus::Pending),
        dismissed_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let saved = model.insert(&db).await.map_err(|e| {
        tracing::error!(error = %e, project_id = %project_id, "Failed to insert draft issue");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    DraftIssueResponse::try_from(saved).map_err(|e| CommandError::Internal { message: e })
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn update_draft_issue(
    state: State<'_, AppState>,
    id: String,
    project_id: Option<String>,
    title: Option<String>,
    body: Option<String>,
    labels: Option<Vec<String>>,
    priority: Option<String>,
    area: Option<String>,
    raw_content: Option<String>,
) -> Result<DraftIssueResponse, CommandError> {
    tracing::info!(id = %id, "Updating draft issue");
    let db = state.get_db().await?;

    let draft_model = item::Entity::find_by_id(&id)
        .filter(item::Column::ItemType.eq("note"))
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query draft issue");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Draft issue".to_string(),
            id: id.clone(),
        })?;

    let now = chrono::Utc::now().naive_utc();

    // Parse existing type_data as NoteData
    let mut note_data = match draft_model
        .parse_type_data()
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        })? {
        ItemTypeData::Note(n) => n,
        _ => {
            return Err(CommandError::Internal {
                message: "Not a note item".to_string(),
            })
        }
    };

    // Apply updates to note_data
    if let Some(l) = labels {
        note_data.labels = Some(l);
    }
    if let Some(p) = priority {
        note_data.priority = Some(p);
    }
    if let Some(a) = area {
        note_data.area = Some(a);
    }
    if let Some(rc) = raw_content {
        note_data.raw_content = rc;
    }

    // Re-serialize
    let type_data = serde_json::to_string(&ItemTypeData::Note(note_data)).map_err(|e| {
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    let mut active: item::ActiveModel = draft_model.into();
    active.updated_at = Set(now);
    active.type_data = Set(type_data);

    if let Some(pid) = project_id {
        active.project_id = Set(pid);
    }
    if let Some(t) = title {
        active.title = Set(t);
    }
    if let Some(b) = body {
        active.body = Set(b);
    }

    let updated = active.update(&db).await.map_err(|e| {
        tracing::error!(error = %e, id = %id, "Failed to update draft issue");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    DraftIssueResponse::try_from(updated).map_err(|e| CommandError::Internal { message: e })
}

#[tauri::command]
pub async fn delete_draft_issue(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), CommandError> {
    tracing::info!(id = %id, "Deleting draft issue");
    let db = state.get_db().await?;

    item::Entity::delete_by_id(&id)
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to delete draft issue");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

#[tauri::command]
pub async fn toggle_draft_issue_star(
    state: State<'_, AppState>,
    id: String,
    is_starred: bool,
) -> Result<(), CommandError> {
    tracing::debug!(id = %id, is_starred = is_starred, "Toggling draft issue star status");
    let db = state.get_db().await?;

    let draft = item::Entity::find_by_id(&id)
        .filter(item::Column::ItemType.eq("note"))
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query draft issue for star status update");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| {
            tracing::warn!(id = %id, "Draft issue not found for star status update");
            CommandError::NotFound {
                entity: "Draft issue".to_string(),
                id: id.clone(),
            }
        })?;

    let mut active: item::ActiveModel = draft.into();
    active.is_starred = Set(is_starred);
    active.update(&db).await.map_err(|e| {
        tracing::error!(error = %e, id = %id, "Failed to update draft issue star status");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    Ok(())
}

#[tauri::command]
pub async fn get_draft_issue_count(state: State<'_, AppState>) -> Result<i64, CommandError> {
    let db = state.get_db().await?;

    let all_notes = item::Entity::find()
        .filter(item::Column::ItemType.eq("note"))
        .all(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to count draft issues");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    // Post-filter: count non-submitted notes
    let count = all_notes
        .into_iter()
        .filter(|m| {
            m.parse_type_data()
                .ok()
                .and_then(|td| match td {
                    ItemTypeData::Note(n) => Some(n.draft_status),
                    _ => None,
                })
                .map(|s| s != DraftIssueStatus::Submitted)
                .unwrap_or(false)
        })
        .count();

    Ok(count as i64)
}

fn build_system_prompt(repo_labels: &[String]) -> String {
    let labels_instruction = if repo_labels.is_empty() {
        "- Labels: Use an empty array for labels since repository labels could not be determined.".to_string()
    } else {
        format!(
            "- Labels: Only use labels from this list: [{}]. If no labels match, use an empty array.",
            repo_labels.join(", ")
        )
    };

    format!(
        r#"You are an AI assistant that converts raw brain dumps into structured GitHub/GitLab issues.

You MUST respond with valid JSON only. No markdown fences, no explanation text — just the JSON object.

Response format:
{{
  "title": "Imperative, specific title (e.g., 'Add rate limiting to /api/upload endpoint')",
  "body": "Markdown body with appropriate sections. For bugs use: ## Steps to Reproduce, ## Expected Behavior, ## Actual Behavior. For features use: ## Description, ## Acceptance Criteria. For tasks use: ## Description, ## Tasks.",
  "labels": ["label1", "label2"],
  "priority": "critical|high|medium|low",
  "area": "backend|frontend|infra|docs|testing|design|other"
}}

Rules:
- Title must be imperative and specific, not vague
- Body should use markdown with clear sections
- Maximum 3 labels
{labels_instruction}
- Infer priority from urgency signals in the text (e.g., "critical", "ASAP", "blocking" → high/critical; no urgency → medium)
- Infer area from technical context clues
- If anything is ambiguous, mark it with [TODO: clarify]
- Keep the body concise but complete"#
    )
}

/// Fetch repository labels from the provider, returning an empty vec on failure.
async fn fetch_repo_labels(
    platform: &ossue_core::enums::Platform,
    owner: &str,
    name: &str,
    token: &str,
    db: &sea_orm::DatabaseConnection,
    project: &ossue_core::models::project::Model,
) -> Vec<String> {
    match platform {
        ossue_core::enums::Platform::GitHub => {
            let base_url = ossue_core::services::auth::get_project_base_url(db, project).await;
            let client = ossue_core::services::github::GitHubClient::with_base_url(
                token.to_string(),
                base_url,
            );
            match client.list_labels(owner, name).await {
                Ok(labels) => labels,
                Err(e) => {
                    tracing::warn!(error = %e, owner = %owner, repo = %name, "Failed to fetch GitHub labels, continuing without");
                    Vec::new()
                }
            }
        }
        ossue_core::enums::Platform::GitLab => {
            let base_url = get_gitlab_base_url(project, db).await;
            let client =
                ossue_core::services::gitlab::GitLabClient::new(token.to_string(), base_url);
            match client.list_labels(owner, name).await {
                Ok(labels) => labels,
                Err(e) => {
                    tracing::warn!(error = %e, owner = %owner, repo = %name, "Failed to fetch GitLab labels, continuing without");
                    Vec::new()
                }
            }
        }
    }
}

#[tauri::command]
pub async fn list_repo_labels(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<String>, CommandError> {
    tracing::debug!(project_id = %project_id, "Fetching repository labels");
    let db = state.get_db().await?;

    let project = ossue_core::models::project::Entity::find_by_id(&project_id)
        .one(&db)
        .await
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Project".to_string(),
            id: project_id.clone(),
        })?;

    let token = match get_project_token(&project, &db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, project_id = %project_id, "Failed to get token for label fetch, returning empty list");
            return Ok(Vec::new());
        }
    };

    let labels = fetch_repo_labels(
        &project.platform,
        &project.owner,
        &project.name,
        &token,
        &db,
        &project,
    )
    .await;

    Ok(labels)
}

#[tauri::command]
pub async fn generate_issue_from_draft(
    state: State<'_, AppState>,
    id: String,
) -> Result<DraftIssueResponse, CommandError> {
    tracing::info!(id = %id, "Generating issue from draft");

    // Load draft and project info
    let (raw_content, project_owner, project_name, project_platform, project_url, token, repo_labels) = {
        let db = state.get_db().await?;

        let draft = item::Entity::find_by_id(&id)
            .filter(item::Column::ItemType.eq("note"))
            .one(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, id = %id, "Failed to query draft issue");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?
            .ok_or_else(|| CommandError::NotFound {
                entity: "Draft issue".to_string(),
                id: id.clone(),
            })?;

        // Parse type_data to get raw_content
        let note_data = match draft
            .parse_type_data()
            .map_err(|e| CommandError::Internal {
                message: e.to_string(),
            })? {
            ItemTypeData::Note(n) => n,
            _ => {
                return Err(CommandError::Internal {
                    message: "Not a note item".to_string(),
                })
            }
        };

        let project = ossue_core::models::project::Entity::find_by_id(&draft.project_id)
            .one(&db)
            .await
            .map_err(|e| CommandError::Internal {
                message: e.to_string(),
            })?
            .ok_or_else(|| CommandError::NotFound {
                entity: "Project".to_string(),
                id: draft.project_id.clone(),
            })?;

        let token = get_project_token(&project, &db).await.unwrap_or_default();

        // Fetch repository labels for the AI prompt
        let labels = fetch_repo_labels(
            &project.platform,
            &project.owner,
            &project.name,
            &token,
            &db,
            &project,
        )
        .await;

        (
            note_data.raw_content.clone(),
            project.owner.clone(),
            project.name.clone(),
            project.platform.clone(),
            project.url.clone(),
            token,
            labels,
        )
    }; // db lock released

    // Build system prompt with repository labels
    let system_prompt = build_system_prompt(&repo_labels);

    // Load AI settings
    let (ai_mode, api_key, ai_model) = {
        let db = state.get_db().await?;

        let ai_mode = get_setting(&db, "ai_mode")
            .await
            .unwrap_or_else(|| "api".to_string());
        let api_key = get_setting(&db, "ai_api_key").await;
        let ai_model = get_setting(&db, "ai_model").await.filter(|s| !s.is_empty());

        (ai_mode, api_key, ai_model)
    }; // db lock released

    // Build user message
    let user_message = format!(
        "Repository: {owner}/{repo}\nPlatform: {platform}\n\n{content}",
        owner = project_owner,
        repo = project_name,
        platform = project_platform,
        content = raw_content,
    );

    let response_text = if ai_mode == "api" {
        // API mode
        let api_key = match api_key {
            Some(key) => key,
            None => {
                reset_draft_status(&state, &id).await;
                return Err(CommandError::AiNotConfigured);
            }
        };

        let service = ossue_core::services::ai_api::AiApiService::new_with_system(
            api_key,
            ai_model.clone(),
            system_prompt.clone(),
        );

        let messages = vec![ossue_core::services::ai_api::ApiMessage {
            role: "user".to_string(),
            content: user_message,
        }];

        match service.send_message(&messages).await {
            Ok((text, _usage)) => text,
            Err(e) => {
                tracing::error!(error = %e, id = %id, "AI API call failed for draft issue generation");
                reset_draft_status(&state, &id).await;
                return Err(CommandError::PlatformApi {
                    message: e.to_string(),
                });
            }
        }
    } else {
        // CLI mode - run claude -p
        tracing::info!(id = %id, "Running Claude CLI for draft issue generation");

        // Fetch repo for CLI context
        let repo_lock_key = format!("{}/{}/{}", project_platform, project_owner, project_name);
        let repo_lock = crate::get_repo_lock(&state.repo_locks, &repo_lock_key).await;

        let repo_path: Option<std::path::PathBuf> = {
            let _repo_guard = repo_lock.lock().await;
            let p = project_platform.clone();
            let o = project_owner.clone();
            let n = project_name.clone();
            let u = project_url.clone();
            let t = token.clone();
            let rm = state.repo_manager.clone();

            let fetch_result =
                tokio::task::spawn_blocking(move || rm.ensure_fetched(&p, &o, &n, &u, &t, false))
                    .await;

            match fetch_result {
                Ok(Ok(path)) => Some(path),
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, id = %id, "Repo fetch failed, continuing without repo context");
                    None
                }
                Err(e) => {
                    tracing::warn!(error = %e, id = %id, "spawn_blocking panicked, continuing without repo context");
                    None
                }
            }
        };

        let mut args = vec![
            "-p".to_string(),
            user_message.clone(),
            "--system-prompt".to_string(),
            system_prompt.clone(),
            "--output-format".to_string(),
            "text".to_string(),
        ];
        if let Some(ref m) = ai_model {
            args.push("--model".to_string());
            args.push(m.clone());
        }

        let mut cmd = tokio::process::Command::new("claude");
        cmd.args(&args);
        if let Some(ref rp) = repo_path {
            cmd.current_dir(rp);
        }

        let output = match cmd.output().await {
            Ok(output) => output,
            Err(e) => {
                tracing::error!(error = %e, "Failed to run claude CLI");
                reset_draft_status(&state, &id).await;
                return Err(CommandError::Internal {
                    message: format!("Failed to run Claude CLI. Is it installed? Error: {e}"),
                });
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(stderr = %stderr, "Claude CLI returned error");
            reset_draft_status(&state, &id).await;
            return Err(CommandError::Internal {
                message: format!("Claude CLI error: {stderr}"),
            });
        }

        String::from_utf8_lossy(&output.stdout).to_string()
    };

    // Parse the AI response as JSON
    tracing::info!(id = %id, raw_response = %response_text, "Raw AI response for draft issue");

    let cleaned = response_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    tracing::info!(id = %id, cleaned_response = %cleaned, "Cleaned AI response for parsing");

    #[derive(Deserialize)]
    struct AiIssueResponse {
        title: Option<String>,
        body: Option<String>,
        labels: Option<Vec<String>>,
        priority: Option<String>,
        area: Option<String>,
    }

    let parsed: AiIssueResponse = serde_json::from_str(cleaned).unwrap_or_else(|e| {
        tracing::warn!(
            error = %e,
            id = %id,
            ai_mode = %ai_mode,
            response_len = response_text.len(),
            cleaned_len = cleaned.len(),
            cleaned_preview = %&cleaned[..cleaned.len().min(200)],
            "Failed to parse AI JSON response, using fallback"
        );
        // Fallback: first line as title, rest as body
        let lines: Vec<&str> = raw_content.lines().collect();
        let title = lines.first().map(|s| s.to_string());
        let body = if lines.len() > 1 {
            Some(lines[1..].join("\n"))
        } else {
            None
        };
        AiIssueResponse {
            title,
            body,
            labels: None,
            priority: None,
            area: None,
        }
    });

    tracing::info!(
        id = %id,
        parsed_title = ?parsed.title,
        parsed_body_len = parsed.body.as_ref().map(|b| b.len()).unwrap_or(0),
        parsed_labels = ?parsed.labels,
        parsed_priority = ?parsed.priority,
        parsed_area = ?parsed.area,
        "Parsed AI response fields"
    );

    // Update draft with AI results
    let db = state.get_db().await?;

    let draft = item::Entity::find_by_id(&id)
        .one(&db)
        .await
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Draft issue".to_string(),
            id: id.clone(),
        })?;

    // Parse existing type_data, update fields, re-serialize
    let mut note_data = match draft
        .parse_type_data()
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        })? {
        ItemTypeData::Note(n) => n,
        _ => {
            return Err(CommandError::Internal {
                message: "Not a note item".to_string(),
            })
        }
    };
    note_data.draft_status = DraftIssueStatus::Ready;
    note_data.labels = parsed.labels;
    note_data.priority = parsed.priority;
    note_data.area = parsed.area;

    let type_data = serde_json::to_string(&ItemTypeData::Note(note_data)).map_err(|e| {
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    let now = chrono::Utc::now().naive_utc();
    let mut active: item::ActiveModel = draft.into();
    active.type_data = Set(type_data);
    active.title = Set(parsed.title.unwrap_or_default());
    active.body = Set(parsed.body.unwrap_or_default());
    active.updated_at = Set(now);

    let updated = active.update(&db).await.map_err(|e| {
        tracing::error!(error = %e, id = %id, "Failed to update draft with AI results");
        CommandError::Internal {
            message: e.to_string(),
        }
    })?;

    tracing::info!(id = %id, "Draft issue generation complete");
    DraftIssueResponse::try_from(updated).map_err(|e| CommandError::Internal { message: e })
}

#[tauri::command]
pub async fn submit_draft_to_provider(
    state: State<'_, AppState>,
    id: String,
) -> Result<CreateIssueResponse, CommandError> {
    tracing::info!(id = %id, "Submitting draft issue to provider");

    let db = state.get_db().await?;

    // 1. Load draft (must be ready)
    let draft = item::Entity::find_by_id(&id)
        .filter(item::Column::ItemType.eq("note"))
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to query draft issue");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Draft issue".to_string(),
            id: id.clone(),
        })?;

    // Parse type_data to check status and get labels
    let note_data = match draft
        .parse_type_data()
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        })? {
        ItemTypeData::Note(n) => n,
        _ => {
            return Err(CommandError::Internal {
                message: "Not a note item".to_string(),
            })
        }
    };

    if note_data.draft_status != DraftIssueStatus::Ready {
        return Err(CommandError::Internal {
            message: "Draft must be in 'ready' status to submit".to_string(),
        });
    }

    let title = if draft.title.is_empty() {
        return Err(CommandError::Internal {
            message: "Draft has no title — generate first".to_string(),
        });
    } else {
        draft.title.clone()
    };

    // 2. Load project
    let project = ossue_core::models::project::Entity::find_by_id(&draft.project_id)
        .one(&db)
        .await
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Project".to_string(),
            id: draft.project_id.clone(),
        })?;

    // 3. Resolve token
    let token = get_project_token(&project, &db).await?;

    // 4. Build request
    let labels = note_data.labels.unwrap_or_default();

    let request = CreateIssueRequest {
        title,
        body: if draft.body.is_empty() {
            None
        } else {
            Some(draft.body.clone())
        },
        labels: if labels.is_empty() {
            None
        } else {
            Some(labels)
        },
    };

    // 5. Call provider (with retry without labels on failure)
    let first_result = match project.platform {
        ossue_core::enums::Platform::GitHub => {
            let base_url =
                ossue_core::services::auth::get_project_base_url(&db, &project).await;
            let client = ossue_core::services::github::GitHubClient::with_base_url(
                token.clone(),
                base_url,
            );
            client
                .create_issue(&project.owner, &project.name, &request)
                .await
        }
        ossue_core::enums::Platform::GitLab => {
            let base_url = get_gitlab_base_url(&project, &db).await;
            let client = ossue_core::services::gitlab::GitLabClient::new(
                token.clone(),
                base_url,
            );
            client
                .create_issue(&project.owner, &project.name, &request)
                .await
        }
    };

    let response: CreateIssueResponse = match first_result {
        Ok(resp) => resp,
        Err(e) if request.labels.is_some() => {
            tracing::warn!(
                error = %e,
                id = %id,
                "Issue creation failed with labels, retrying without labels"
            );
            let request_without_labels = CreateIssueRequest {
                title: request.title.clone(),
                body: request.body.clone(),
                labels: None,
            };
            match project.platform {
                ossue_core::enums::Platform::GitHub => {
                    let base_url =
                        ossue_core::services::auth::get_project_base_url(&db, &project).await;
                    let client = ossue_core::services::github::GitHubClient::with_base_url(
                        token.clone(),
                        base_url,
                    );
                    client
                        .create_issue(&project.owner, &project.name, &request_without_labels)
                        .await
                        .map_err(|e| CommandError::PlatformApi {
                            message: e.to_string(),
                        })?
                }
                ossue_core::enums::Platform::GitLab => {
                    let base_url = get_gitlab_base_url(&project, &db).await;
                    let client = ossue_core::services::gitlab::GitLabClient::new(
                        token.clone(),
                        base_url,
                    );
                    client
                        .create_issue(&project.owner, &project.name, &request_without_labels)
                        .await
                        .map_err(|e| CommandError::PlatformApi {
                            message: e.to_string(),
                        })?
                }
            }
        }
        Err(e) => {
            return Err(CommandError::PlatformApi {
                message: e.to_string(),
            });
        }
    };

    // 6. Delete draft from DB
    item::Entity::delete_by_id(&id)
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, id = %id, "Failed to delete draft after submission");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    tracing::info!(id = %id, number = response.number, url = %response.url, "Draft submitted successfully");
    Ok(response)
}

/// Helper to get project token (direct or via connector)
async fn get_project_token(
    project: &ossue_core::models::project::Model,
    db: &sea_orm::DatabaseConnection,
) -> Result<String, CommandError> {
    use ossue_core::models::{connector, settings};

    if let Some(token) = &project.api_token {
        return Ok(token.clone());
    }
    if let Some(connector_id) = &project.connector_id {
        let conn = connector::Entity::find_by_id(connector_id)
            .one(db)
            .await
            .map_err(|e| CommandError::Internal {
                message: e.to_string(),
            })?
            .ok_or_else(|| CommandError::NotFound {
                entity: "Connector".to_string(),
                id: connector_id.clone(),
            })?;
        return Ok(conn.token);
    }

    let token_key = match project.platform {
        ossue_core::enums::Platform::GitHub => "github_token",
        ossue_core::enums::Platform::GitLab => "gitlab_token",
    };
    let setting = settings::Entity::find_by_id(token_key)
        .one(db)
        .await
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        })?
        .ok_or_else(|| CommandError::Internal {
            message: "No API token configured for this project".to_string(),
        })?;
    Ok(setting.value)
}

async fn get_setting(db: &sea_orm::DatabaseConnection, key: &str) -> Option<String> {
    use ossue_core::models::settings;
    settings::Entity::find_by_id(key)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|s| s.value)
}

async fn reset_draft_status(state: &AppState, id: &str) {
    if let Some(db) = state.db.read().await.clone() {
        if let Ok(Some(draft)) = item::Entity::find_by_id(id).one(&db).await {
            // Parse type_data, update draft_status to Draft, re-serialize
            if let Ok(ItemTypeData::Note(mut note_data)) = draft.parse_type_data() {
                note_data.draft_status = DraftIssueStatus::Draft;
                if let Ok(type_data) = serde_json::to_string(&ItemTypeData::Note(note_data)) {
                    let mut active: item::ActiveModel = draft.into();
                    active.type_data = Set(type_data);
                    active.updated_at = Set(chrono::Utc::now().naive_utc());
                    let _ = active.update(&db).await;
                }
            }
        }
    }
}

async fn get_gitlab_base_url(
    project: &ossue_core::models::project::Model,
    db: &sea_orm::DatabaseConnection,
) -> Option<String> {
    use ossue_core::models::connector;
    if let Some(connector_id) = &project.connector_id {
        if let Ok(Some(conn)) = connector::Entity::find_by_id(connector_id).one(db).await {
            return conn.base_url;
        }
    }
    None
}
