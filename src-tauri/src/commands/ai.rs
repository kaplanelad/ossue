use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use uuid::Uuid;

use super::error::CommandError;
use crate::AppState;
use ossue_core::enums::ActionType;
use ossue_core::models::chat_message;
use ossue_core::models::project_settings;
use ossue_core::models::settings as settings_model;
use ossue_core::services::ai_api::{AiApiService, ApiMessage};
use ossue_core::services::context::ContextService;

// ---------------------------------------------------------------------------
// AnalysisContext: encapsulates the common "gather data" steps shared by
// send_chat_message and analyze_item_action.
// ---------------------------------------------------------------------------

/// Everything needed to run an AI analysis, gathered from the database.
struct AnalysisContext {
    item: ossue_core::models::item::Model,
    project: ossue_core::models::project::Model,
    token: String,
    ai_mode: String,
    api_key_value: Option<String>,
    ai_model: Option<String>,
    custom_instructions: Option<String>,
    focus_areas_raw: Option<String>,
    review_strictness: Option<String>,
    response_tone: Option<String>,
    github_base_url: Option<String>,
}

impl AnalysisContext {
    /// Gathers item, project, token, AI settings, and resolved preferences
    /// from the database. This replaces the duplicated "gather context" blocks
    /// in send_chat_message and analyze_item_action (steps 1-7).
    async fn build(db: &DatabaseConnection, item_id: &str) -> Result<Self, CommandError> {
        // 1. Load item
        let item = ossue_core::models::item::Entity::find_by_id(item_id)
            .one(db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, item_id = %item_id, "Failed to query item");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?
            .ok_or_else(|| {
                tracing::warn!(item_id = %item_id, "Item not found");
                CommandError::NotFound {
                    entity: "Item".to_string(),
                    id: item_id.to_string(),
                }
            })?;

        // 2. Load project
        let project =
            ossue_core::models::project::Entity::find_by_id(&item.project_id)
                .one(db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, project_id = %item.project_id, "Failed to query project");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })?
                .ok_or_else(|| {
                    tracing::warn!(project_id = %item.project_id, "Project not found");
                    CommandError::NotFound {
                        entity: "Project".to_string(),
                        id: item.project_id.clone(),
                    }
                })?;

        // 3. Get token
        let token = get_project_token(&project, db).await?;

        // 4. Get AI settings (global)
        let ai_mode = get_setting(db, "ai_mode")
            .await
            .unwrap_or_else(|| "api".to_string());
        let api_key_value = get_setting(db, "ai_api_key").await;
        let ai_model = get_setting(db, "ai_model").await.filter(|s| !s.is_empty());
        let custom_instructions = get_setting(db, "custom_instructions").await;

        // 5. Load global AI preferences
        let global_focus_areas = get_setting(db, "ai_focus_areas").await;
        let global_review_strictness = get_setting(db, "ai_review_strictness").await;
        let global_response_tone = get_setting(db, "ai_response_tone").await;

        // 6. Load per-project settings overrides
        let proj_settings = project_settings::Entity::find()
            .filter(project_settings::Column::ProjectId.eq(&project.id))
            .all(db)
            .await
            .unwrap_or_default();

        let proj_setting = |key: &str| -> Option<String> {
            proj_settings
                .iter()
                .find(|s| s.key == key)
                .map(|s| s.value.clone())
        };

        // 7. Resolve preferences: project override > global
        let focus_areas_raw = proj_setting("ai_focus_areas").or(global_focus_areas);
        let review_strictness = proj_setting("ai_review_strictness").or(global_review_strictness);
        let response_tone = proj_setting("ai_response_tone").or(global_response_tone);

        // 8. Get GitHub base URL for API calls
        let github_base_url =
            ossue_core::services::auth::get_project_base_url(db, &project).await;

        Ok(Self {
            item,
            project,
            token,
            ai_mode,
            api_key_value,
            ai_model,
            custom_instructions,
            focus_areas_raw,
            review_strictness,
            response_tone,
            github_base_url,
        })
    }

    /// Build the system prompt including user preferences (custom instructions,
    /// strictness, tone, focus areas). Used by send_chat_message.
    fn build_system_prompt(&self, action_type: &ActionType) -> String {
        let mut system_prompt =
            ContextService::build_system_prompt(action_type, &self.item.item_type);

        if let Some(ref instructions) = self.custom_instructions {
            if !instructions.is_empty() {
                system_prompt.push_str(&format!("\n\n## Custom Instructions\n{}", instructions));
            }
        }
        if let Some(ref strictness) = self.review_strictness {
            system_prompt.push_str(&format!("\n\n## Review Strictness\n{}", strictness));
        }
        if let Some(ref tone) = self.response_tone {
            system_prompt.push_str(&format!("\n\n## Response Tone\n{}", tone));
        }
        if let Some(ref areas) = self.focus_areas_raw {
            let focus_areas: Vec<String> = serde_json::from_str::<Vec<String>>(areas)
                .unwrap_or_else(|_| {
                    areas
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                });
            if !focus_areas.is_empty() {
                system_prompt.push_str(&format!(
                    "\n\n## Focus Areas\n- {}",
                    focus_areas.join("\n- ")
                ));
            }
        }

        system_prompt
    }

    /// Build an item context block to append to the system prompt.
    /// When `fresh_diff` is provided it takes precedence over the DB-cached diff.
    fn build_item_context_block(&self, fresh_diff: Option<&str>) -> String {
        let td = match self.item.parse_type_data() {
            Ok(td) => td,
            Err(_) => return String::new(),
        };

        let mut sections = Vec::new();

        sections.push(format!(
            "## Item Context\n\
             - **Type:** {}\n\
             - **Title:** {}\n\
             - **Author:** {}\n\
             - **State:** {}\n\
             - **URL:** {}",
            self.item.item_type,
            self.item.title,
            td.author().unwrap_or(""),
            td.state().map(|s| format!("{s:?}")).unwrap_or_default(),
            td.url().unwrap_or(""),
        ));

        if !self.item.body.is_empty() {
            sections.push(format!("## Description\n{}", self.item.body));
        }

        // Use fresh diff if provided, otherwise fall back to DB-cached diff
        let diff_to_use = fresh_diff.or_else(|| {
            if let ossue_core::enums::ItemTypeData::Pr(ref pr) = td {
                pr.pr_diff.as_deref()
            } else {
                None
            }
        });
        if let Some(diff) = diff_to_use {
            if !diff.is_empty() {
                sections.push(format!("## Diff\n```diff\n{}\n```", diff));
            }
        }

        if sections.is_empty() {
            String::new()
        } else {
            format!("\n\n{}", sections.join("\n\n"))
        }
    }

    /// Parse focus_areas_raw into a Vec<String>. Used by analyze_item_action
    /// to populate ItemContext.
    fn parse_focus_areas(&self) -> Vec<String> {
        match self.focus_areas_raw {
            Some(ref areas) => serde_json::from_str::<Vec<String>>(areas).unwrap_or_else(|_| {
                areas
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }),
            None => Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessageResponse {
    pub id: String,
    pub item_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeActionRequest {
    pub item_id: String,
    pub action: String, // "analyze", "draft_response"
}

#[tauri::command]
pub async fn get_chat_messages(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<Vec<ChatMessageResponse>, CommandError> {
    let db = state.get_db().await?;

    let messages = chat_message::Entity::find()
        .filter(chat_message::Column::ItemId.eq(&item_id))
        .order_by_asc(chat_message::Column::CreatedAt)
        .all(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, item_id = %item_id, "Failed to query chat messages");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    tracing::debug!(item_id = %item_id, count = messages.len(), "Retrieved chat messages");

    Ok(messages
        .into_iter()
        .map(|m| ChatMessageResponse {
            id: m.id,
            item_id: m.item_id,
            role: m.role,
            content: m.content,
            created_at: m.created_at.to_string(),
            input_tokens: m.input_tokens,
            output_tokens: m.output_tokens,
            model: m.model,
        })
        .collect())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(item_id = %item_id))]
pub async fn send_chat_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    item_id: String,
    message: String,
) -> Result<ChatMessageResponse, CommandError> {
    use ossue_core::models::analysis_history;
    use ossue_core::services::repo_manager::RepoManager;

    tracing::info!(item_id = %item_id, "Sending chat message");
    let _ = app.emit(
        "ai-analysis-progress",
        serde_json::json!({
            "item_id": &item_id,
            "status": "Preparing message..."
        }),
    );

    // Gather all data we need while holding the DB lock
    let (ctx, action_type, api_messages) = {
        let db = state.get_db().await?;

        let now = chrono::Utc::now().naive_utc();

        // Save user message
        let user_msg = chat_message::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            item_id: Set(item_id.clone()),
            role: Set("user".to_string()),
            content: Set(message.clone()),
            created_at: Set(now),
            input_tokens: Set(None),
            output_tokens: Set(None),
            model: Set(None),
        };
        user_msg.insert(&db).await.map_err(|e| {
            tracing::error!(error = %e, item_id = %item_id, "Failed to save user message to database");
            CommandError::Internal { message: e.to_string() }
        })?;

        // Steps 1-7: load item, project, token, AI settings, preferences
        let ctx = AnalysisContext::build(&db, &item_id).await?;

        // 8. Get last action type from analysis_history
        let last_action = analysis_history::Entity::find()
            .filter(analysis_history::Column::ItemId.eq(&item_id))
            .order_by_desc(analysis_history::Column::CreatedAt)
            .one(&db)
            .await
            .ok()
            .flatten();

        let action_type = last_action
            .map(|h| h.action_type)
            .unwrap_or(ActionType::Analyze);

        // 9. Get conversation history
        let history = chat_message::Entity::find()
            .filter(chat_message::Column::ItemId.eq(&item_id))
            .order_by_asc(chat_message::Column::CreatedAt)
            .all(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, item_id = %item_id, "Failed to query chat history");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;

        let api_messages: Vec<ApiMessage> = history
            .iter()
            .map(|m| ApiMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();
        let api_messages = truncate_chat_history(api_messages);

        (ctx, action_type, api_messages)
    }; // db lock released here

    tracing::debug!(item_id = %item_id, message_count = api_messages.len(), ai_mode = %ctx.ai_mode, "Conversation history prepared");

    // Fetch fresh PR diff from GitHub API (same pattern as auto_analyze_item)
    let fresh_diff = if ctx.item.item_type == ossue_core::enums::ItemType::PullRequest {
        let td = ctx.item.parse_type_data().ok();
        let cached_diff = match &td {
            Some(ossue_core::enums::ItemTypeData::Pr(pr)) => pr.pr_diff.clone(),
            _ => None,
        };
        let ext_id = td.as_ref().and_then(|t| t.external_id());
        if let Some(ext_id) = ext_id {
            match ctx.project.platform {
                ossue_core::enums::Platform::GitHub => {
                    let client = ossue_core::services::github::GitHubClient::with_base_url(
                        ctx.token.clone(),
                        ctx.github_base_url.clone(),
                    );
                    match client
                        .get_pr_diff(&ctx.project.owner, &ctx.project.name, ext_id)
                        .await
                    {
                        Ok(diff) => {
                            let truncated = if diff.len() > 200_000 {
                                diff[..200_000].to_string()
                            } else {
                                diff
                            };
                            Some(truncated)
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, item_id = %item_id, "Failed to fetch fresh PR diff for chat, falling back to cached");
                            cached_diff
                        }
                    }
                }
                ossue_core::enums::Platform::GitLab => cached_diff,
            }
        } else {
            cached_diff
        }
    } else {
        None
    };

    // Build system prompt with user preferences and item context so the LLM
    // always knows which PR/issue is being discussed, even without prior analysis.
    let mut system_prompt = ctx.build_system_prompt(&action_type);
    system_prompt.push_str(&ctx.build_item_context_block(fresh_diff.as_deref()));

    // Determine PR number and default branch for correct worktree checkout
    let td = ctx.item.parse_type_data().ok();
    let pr_number = if ctx.item.item_type == ossue_core::enums::ItemType::PullRequest {
        td.as_ref().and_then(|t| t.external_id())
    } else {
        None
    };
    let default_branch = ctx.project.default_branch.clone();

    // Prepare repo path for CLI mode - force-fetch and create worktree for fresh context
    let (repo_path, chat_worktree): (
        Option<std::path::PathBuf>,
        Option<ossue_core::services::repo_manager::AnalysisWorktree>,
    ) = if ctx.ai_mode != "api" {
        let repo_lock_key = format!(
            "{}/{}/{}",
            ctx.project.platform, ctx.project.owner, ctx.project.name
        );
        let repo_lock = crate::get_repo_lock(&state.repo_locks, &repo_lock_key).await;

        let fetch_result = {
            let _repo_guard = repo_lock.lock().await;
            tracing::debug!(repo = %repo_lock_key, "Acquired repo lock for fetch (follow-up)");

            let p = ctx.project.platform.clone();
            let o = ctx.project.owner.clone();
            let n = ctx.project.name.clone();
            let u = ctx.project.url.clone();
            let t = ctx.token.clone();
            let rm = state.repo_manager.clone();

            tokio::task::spawn_blocking(move || rm.ensure_fetched(&p, &o, &n, &u, &t, true)).await
        };

        match fetch_result {
            Ok(Ok(path)) => {
                // Create a worktree for CLI mode so it sees proper file state
                let wt_path = path.clone();
                let wt_item_type = ctx.item.item_type.clone();
                let wt_token = ctx.token.clone();
                let wt_db = default_branch.clone();
                let wt_result = tokio::task::spawn_blocking(move || {
                    RepoManager::create_analysis_worktree(
                        &wt_path,
                        &wt_item_type,
                        pr_number,
                        wt_db.as_deref(),
                        &wt_token,
                    )
                })
                .await;

                match wt_result {
                    Ok(Ok(wt)) => {
                        let wt_path = wt.worktree_path.clone();
                        (Some(wt_path), Some(wt))
                    }
                    _ => {
                        tracing::warn!(item_id = %item_id, "Worktree creation failed for follow-up, using repo path");
                        (Some(path), None)
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, item_id = %item_id, "Repo fetch failed for follow-up, continuing without repo context");
                (None, None)
            }
            Err(e) => {
                tracing::warn!(error = %e, item_id = %item_id, "spawn_blocking panicked for repo fetch, continuing without repo context");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    let (response_content, input_tokens, output_tokens, model_used) = if ctx.ai_mode == "api" {
        let api_key_value = ctx.api_key_value.ok_or_else(|| {
            tracing::error!(item_id = %item_id, "AI API key not configured");
            CommandError::AiNotConfigured
        })?;

        let service =
            AiApiService::new_with_system(api_key_value, ctx.ai_model.clone(), system_prompt);
        let model_name = service.model().to_string();

        // Emit streaming start event
        tracing::info!(item_id = %item_id, "Starting AI streaming");
        let _ = app.emit(
            "ai-analysis-progress",
            serde_json::json!({
                "item_id": &item_id,
                "status": "Waiting for AI..."
            }),
        );
        let _ = app.emit("ai-stream-start", &item_id);

        let (full_response, input_tokens, output_tokens) =
            stream_llm_response(&app, &item_id, &service, &api_messages).await?;
        let _ = app.emit("ai-stream-end", &item_id);
        (full_response, input_tokens, output_tokens, Some(model_name))
    } else {
        // CLI mode - run claude -p with the conversation as prompt
        tracing::info!(item_id = %item_id, "Running Claude CLI for analysis");

        // Prepend system prompt to conversation text
        let mut prompt_text = format!("System: {}\n\n", system_prompt);
        for msg in &api_messages {
            let role_label = if msg.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            prompt_text.push_str(&format!("{role_label}: {}\n\n", msg.content));
        }

        let mut args = vec!["-p".to_string(), prompt_text];
        if let Some(ref m) = ctx.ai_model {
            args.push("--model".to_string());
            args.push(m.clone());
        }

        let mut cmd = tokio::process::Command::new("claude");
        cmd.args(&args);
        if let Some(ref rp) = repo_path {
            cmd.current_dir(rp);
        }

        let output = cmd.output().await.map_err(|e| {
            tracing::error!(error = %e, item_id = %item_id, "Failed to run claude CLI");
            CommandError::Internal {
                message: format!("Failed to run Claude CLI. Is it installed? Error: {e}"),
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(stderr = %stderr, item_id = %item_id, "Claude CLI returned error");
            return Err(CommandError::Internal {
                message: format!("Claude CLI error: {stderr}"),
            });
        }

        let response = String::from_utf8_lossy(&output.stdout).to_string();
        tracing::info!(item_id = %item_id, response_len = response.len(), "Claude CLI completed");
        (response, None, None, ctx.ai_model)
    };

    // Save assistant message (re-acquire DB lock)
    let db = state.get_db().await?;

    let assistant_msg = chat_message::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        item_id: Set(item_id.clone()),
        role: Set("assistant".to_string()),
        content: Set(response_content.clone()),
        created_at: Set(chrono::Utc::now().naive_utc()),
        input_tokens: Set(input_tokens),
        output_tokens: Set(output_tokens),
        model: Set(model_used.clone()),
    };
    let saved = assistant_msg.insert(&db).await.map_err(|e| {
        tracing::error!(error = %e, item_id = %item_id, "Failed to save assistant message to database");
        CommandError::Internal { message: e.to_string() }
    })?;

    // Cleanup worktree if we created one for CLI mode
    if let Some(wt) = chat_worktree {
        let wt_clone = wt.clone();
        tokio::task::spawn_blocking(move || {
            RepoManager::cleanup_worktree(&wt_clone);
        })
        .await
        .ok();
    }

    Ok(ChatMessageResponse {
        id: saved.id,
        item_id: saved.item_id,
        role: saved.role,
        content: saved.content,
        created_at: saved.created_at.to_string(),
        input_tokens: saved.input_tokens,
        output_tokens: saved.output_tokens,
        model: saved.model,
    })
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(item_id = %item_id))]
pub async fn auto_analyze_item(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    item_id: String,
) -> Result<ChatMessageResponse, CommandError> {
    use ossue_core::enums::Platform;

    tracing::info!(item_id = %item_id, "Auto-analyzing item");

    let (item, project, token, github_base_url) = {
        let db = state.get_db().await?;

        // Check if already analyzed
        let existing = chat_message::Entity::find()
            .filter(chat_message::Column::ItemId.eq(&item_id))
            .one(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, item_id = %item_id, "Failed to query existing analysis");
                CommandError::Internal { message: e.to_string() }
            })?;

        if existing.is_some() {
            tracing::warn!(item_id = %item_id, "Item already analyzed, skipping");
            return Err(CommandError::Internal {
                message: "Already analyzed".to_string(),
            });
        }

        let item = ossue_core::models::item::Entity::find_by_id(&item_id)
            .one(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, item_id = %item_id, "Failed to query item for auto-analysis");
                CommandError::Internal { message: e.to_string() }
            })?
            .ok_or_else(|| {
                tracing::warn!(item_id = %item_id, "Item not found for auto-analysis");
                CommandError::NotFound {
                    entity: "Item".to_string(),
                    id: item_id.clone(),
                }
            })?;

        let project = ossue_core::models::project::Entity::find_by_id(&item.project_id)
            .one(&db)
            .await
            .map_err(|e| CommandError::Internal {
                message: e.to_string(),
            })?
            .ok_or_else(|| CommandError::NotFound {
                entity: "Project".to_string(),
                id: item.project_id.clone(),
            })?;

        let token = get_project_token(&project, &db).await?;
        let github_base_url = ossue_core::services::auth::get_project_base_url(&db, &project).await;

        (item, project, token, github_base_url)
    }; // db lock released

    // Fetch fresh PR diff from API if this is a PR
    let td = item.parse_type_data().map_err(|e| CommandError::Internal {
        message: e.to_string(),
    })?;
    let fresh_diff = if item.item_type == ossue_core::enums::ItemType::PullRequest {
        let cached_diff = match &td {
            ossue_core::enums::ItemTypeData::Pr(pr) => pr.pr_diff.clone(),
            _ => None,
        };
        if let Some(ext_id) = td.external_id() {
            match project.platform {
                Platform::GitHub => {
                    let client = ossue_core::services::github::GitHubClient::with_base_url(
                        token.clone(),
                        github_base_url.clone(),
                    );
                    match client
                        .get_pr_diff(&project.owner, &project.name, ext_id)
                        .await
                    {
                        Ok(diff) => {
                            let truncated = if diff.len() > 200_000 {
                                diff[..200_000].to_string()
                            } else {
                                diff
                            };
                            Some(truncated)
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, item_id = %item_id, "Failed to fetch fresh PR diff, falling back to cached");
                            cached_diff
                        }
                    }
                }
                Platform::GitLab => {
                    // GitLab doesn't have a diff endpoint; use cached if available
                    cached_diff
                }
            }
        } else {
            cached_diff
        }
    } else {
        None
    };

    let prompt = AiApiService::build_analysis_prompt(
        &item.item_type,
        &item.title,
        &item.body,
        fresh_diff.as_deref(),
    );

    send_chat_message(app, state, item_id, prompt).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(item_id = %request.item_id, action = %request.action))]
pub async fn analyze_item_action(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: AnalyzeActionRequest,
) -> Result<ChatMessageResponse, CommandError> {
    use ossue_core::enums::Platform;
    use ossue_core::models::analysis_history;
    use ossue_core::models::project_note;
    use ossue_core::services::repo_manager::RepoManager;

    tracing::info!(item_id = %request.item_id, action = %request.action, "Starting action-based analysis");

    let _ = app.emit(
        "ai-analysis-progress",
        serde_json::json!({
            "item_id": &request.item_id,
            "status": "Loading item data..."
        }),
    );

    // Parse action string into ActionType enum
    let action_type = match request.action.as_str() {
        "analyze" => ActionType::Analyze,
        "draft_response" => ActionType::DraftResponse,
        _ => {
            return Err(CommandError::Internal {
                message: format!("Unknown action: {}", request.action),
            })
        }
    };

    // Gather all data we need while holding the DB lock
    let (ctx, maintainer_notes, gitlab_base_url, github_base_url, chat_history) = {
        let db = state.get_db().await?;

        // Steps 1-7: load item, project, token, AI settings, preferences
        let ctx = AnalysisContext::build(&db, &request.item_id).await?;

        // 8. Get maintainer notes
        let notes = project_note::Entity::find()
            .filter(project_note::Column::ProjectId.eq(&ctx.project.id))
            .all(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, project_id = %ctx.project.id, "Failed to query project notes");
                CommandError::Internal { message: e.to_string() }
            })?;
        let mut maintainer_notes: Vec<String> = notes.into_iter().map(|n| n.content).collect();

        // 9. Get draft issues (notes) for additional context
        let draft_notes = ossue_core::models::item::Entity::find()
            .filter(ossue_core::models::item::Column::ProjectId.eq(&ctx.project.id))
            .filter(ossue_core::models::item::Column::ItemType.eq("note"))
            .all(&db)
            .await
            .unwrap_or_default();

        for note in draft_notes.into_iter().take(10) {
            let note_td = match note.parse_type_data() {
                Ok(ossue_core::enums::ItemTypeData::Note(nd)) => nd,
                _ => continue,
            };
            if note_td.draft_status == ossue_core::enums::DraftIssueStatus::Submitted {
                continue;
            }
            let title_part = if note.title.is_empty() {
                ""
            } else {
                note.title.as_str()
            };
            let content_part: String = note_td.raw_content.chars().take(500).collect();
            let entry = if title_part.is_empty() {
                format!("[Draft Note] {content_part}")
            } else {
                format!("[Draft Note: {title_part}] {content_part}")
            };
            maintainer_notes.push(entry);
        }

        // Get base_url for platform
        let gitlab_base_url = get_gitlab_base_url(&ctx.project, &db).await;
        let github_base_url =
            ossue_core::services::auth::get_project_base_url(&db, &ctx.project).await;

        // Get existing chat history for this item
        let chat_history = chat_message::Entity::find()
            .filter(chat_message::Column::ItemId.eq(&request.item_id))
            .order_by_asc(chat_message::Column::CreatedAt)
            .all(&db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, item_id = %request.item_id, "Failed to query chat history");
                CommandError::Internal { message: e.to_string() }
            })?;

        (
            ctx,
            maintainer_notes,
            gitlab_base_url,
            github_base_url,
            chat_history,
        )
    }; // db lock released here

    // Phase 1: Short lock for fetch only
    let _ = app.emit(
        "ai-analysis-progress",
        serde_json::json!({
            "item_id": &request.item_id,
            "status": "Preparing repository..."
        }),
    );

    let td = ctx
        .item
        .parse_type_data()
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        })?;

    let platform = ctx.project.platform.clone();
    let owner = ctx.project.owner.clone();
    let name = ctx.project.name.clone();
    let url = ctx.project.url.clone();
    let token_clone = ctx.token.clone();
    let item_type = ctx.item.item_type.clone();
    let pr_number = if ctx.item.item_type == ossue_core::enums::ItemType::PullRequest {
        td.external_id()
    } else {
        None
    };
    let default_branch = ctx.project.default_branch.clone();

    // Acquire repo lock for fetch only (short duration)
    let repo_lock_key = format!(
        "{}/{}/{}",
        ctx.project.platform, ctx.project.owner, ctx.project.name
    );
    let repo_lock = crate::get_repo_lock(&state.repo_locks, &repo_lock_key).await;

    let repo_path = {
        let _repo_guard = repo_lock.lock().await;
        tracing::debug!(repo = %repo_lock_key, "Acquired repo lock for fetch");

        let p = platform.clone();
        let o = owner.clone();
        let n = name.clone();
        let u = url.clone();
        let t = token_clone.clone();
        let rm = state.repo_manager.clone();

        tokio::task::spawn_blocking(move || rm.ensure_fetched(&p, &o, &n, &u, &t, true))
            .await
            .map_err(|e| {
                tracing::error!(error = %e, item_id = %request.item_id, "spawn_blocking panicked for repo fetch");
                CommandError::Internal { message: format!("Failed to prepare repo: {e}") }
            })?
            .map_err(|e| {
                tracing::error!(error = %e, item_id = %request.item_id, "Repo fetch failed");
                let _ = app.emit(
                    "ai-analysis-progress",
                    serde_json::json!({
                        "item_id": &request.item_id,
                        "status": "Warning: Could not fetch latest code. Analysis may use stale data."
                    }),
                );
                CommandError::Internal { message: e.to_string() }
            })
        // Lock released here when _repo_guard drops
    };

    // Phase 2: Create worktree (no lock needed)
    let worktree = if let Ok(ref rp) = repo_path {
        let rp = rp.clone();
        let it = item_type.clone();
        let db = default_branch.clone();
        let tc = ctx.token.clone();

        let wt_result = tokio::task::spawn_blocking(move || {
            RepoManager::create_analysis_worktree(&rp, &it, pr_number, db.as_deref(), &tc)
        })
        .await
        .map_err(|e| {
            tracing::error!(error = %e, item_id = %request.item_id, "spawn_blocking panicked for worktree creation");
            CommandError::Internal { message: format!("Failed to create worktree: {e}") }
        })?;

        match wt_result {
            Ok(wt) => Some(wt),
            Err(e) => {
                tracing::warn!(error = %e, item_id = %request.item_id, "Worktree creation failed, falling back to repo path");
                None
            }
        }
    } else {
        None
    };

    // Determine the path to use for context gathering and AI execution
    let analysis_path = worktree
        .as_ref()
        .map(|wt| wt.worktree_path.clone())
        .or_else(|| repo_path.as_ref().ok().cloned());
    let analysis_path_ref = analysis_path.as_deref();

    // Phase 3: Gather context (no lock needed)
    let _ = app.emit(
        "ai-analysis-progress",
        serde_json::json!({
            "item_id": &request.item_id,
            "status": "Fetching project data..."
        }),
    );
    let ext_id = td.external_id().unwrap_or(0);
    let mut item_context = match ctx.project.platform {
        Platform::GitHub => {
            let client = ossue_core::services::github::GitHubClient::with_base_url(
                ctx.token.clone(),
                github_base_url.clone(),
            );
            ContextService::gather_github_context(
                &client,
                &ctx.project.owner,
                &ctx.project.name,
                &ctx.item.item_type,
                ext_id,
                analysis_path_ref,
                ctx.project.default_branch.as_deref(),
            )
            .await
        }
        Platform::GitLab => {
            let client =
                ossue_core::services::gitlab::GitLabClient::new(ctx.token.clone(), gitlab_base_url);
            let gitlab_project_id = ctx.project.external_project_id.ok_or_else(|| {
                if let Some(ref wt) = worktree {
                    RepoManager::cleanup_worktree(wt);
                }
                CommandError::Internal {
                    message: "GitLab project ID not cached. Sync the project first.".to_string(),
                }
            })?;
            ContextService::gather_gitlab_context(
                &client,
                gitlab_project_id,
                &ctx.item.item_type,
                ext_id,
                analysis_path_ref,
                ctx.project.default_branch.as_deref(),
            )
            .await
        }
    };

    // Cache fetched PR diff in DB so auto_analyze_item can use it later
    let cached_pr_diff = match &td {
        ossue_core::enums::ItemTypeData::Pr(pr) => pr.pr_diff.clone(),
        _ => None,
    };
    if item_context.pr_diff.is_some() && cached_pr_diff.is_none() {
        if let Some(db) = state.db.read().await.clone() {
            use ossue_core::models::item;
            // Update type_data JSON with the new pr_diff
            let mut updated_td = td.clone();
            if let ossue_core::enums::ItemTypeData::Pr(ref mut pr) = updated_td {
                pr.pr_diff = item_context.pr_diff.clone();
            }
            let mut active: item::ActiveModel = ctx.item.clone().into();
            active.type_data = Set(serde_json::to_string(&updated_td).unwrap());
            if let Err(e) = active.update(&db).await {
                tracing::warn!(error = %e, item_id = %request.item_id, "Failed to cache PR diff in DB");
            } else {
                tracing::debug!(item_id = %request.item_id, "Cached PR diff in DB");
            }
        }
    }

    // Populate ItemContext with item details and resolved settings
    item_context.title = ctx.item.title.clone();
    item_context.body = ctx.item.body.clone();
    item_context.author = td.author().unwrap_or("").to_string();
    item_context.url = td.url().unwrap_or("").to_string();
    item_context.state = td.state().map(|s| s.to_string()).unwrap_or_default();
    item_context.maintainer_notes = maintainer_notes;
    item_context.focus_areas = ctx.parse_focus_areas();
    item_context.custom_instructions = ctx.custom_instructions;
    item_context.review_strictness = ctx.review_strictness;
    item_context.response_tone = ctx.response_tone;

    // Build prompt and compute hash for analysis history
    let _ = app.emit(
        "ai-analysis-progress",
        serde_json::json!({
            "item_id": &request.item_id,
            "status": "Building analysis context..."
        }),
    );
    let action_prompt = ContextService::build_action_prompt(
        &action_type,
        &item_context,
        item_context.pr_diff.as_deref(),
    );
    let prompt_hash = format!("{:x}", md5::compute(&action_prompt));

    // User-visible label for the chat — don't show the raw prompt
    let display_label = match action_type {
        ActionType::Analyze => "Analyze".to_string(),
        ActionType::DraftResponse => "Draft Response".to_string(),
    };

    // Save analysis history record
    {
        let db = state.get_db().await?;

        let provider_mode = if ctx.ai_mode == "api" {
            ossue_core::enums::ProviderMode::Api
        } else {
            ossue_core::enums::ProviderMode::Cli
        };

        let history_record = analysis_history::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            item_id: Set(request.item_id.clone()),
            action_type: Set(action_type.clone()),
            provider_mode: Set(provider_mode),
            prompt_hash: Set(prompt_hash),
            created_at: Set(chrono::Utc::now().naive_utc()),
        };
        history_record.insert(&db).await.map_err(|e| {
            tracing::error!(error = %e, item_id = %request.item_id, "Failed to save analysis history");
            CommandError::Internal { message: e.to_string() }
        })?;
    }

    // Phase 4: Run AI analysis
    let result = if ctx.ai_mode != "api" {
        // CLI mode - needs the worktree/repo path
        let cli_path = analysis_path.ok_or_else(|| {
            if let Some(ref wt) = worktree {
                RepoManager::cleanup_worktree(wt);
            }
            CommandError::Internal {
                message: "Could not prepare repository for deep analysis. \
                 Try syncing the project first or switching to API mode in AI Configuration."
                    .to_string(),
            }
        })?;

        let cli_tool = ossue_core::services::provider::CliTool::from_str(&ctx.ai_mode)
            .unwrap_or(ossue_core::services::provider::CliTool::ClaudeCode);

        if action_type == ActionType::Analyze {
            // Multi-step CLI analysis
            let steps = ContextService::build_analysis_steps(
                &ctx.item.item_type,
                &item_context,
                item_context.pr_diff.as_deref(),
            );

            let system_prompt = ContextService::build_multi_step_system_prompt(&ctx.item.item_type);
            let mut conversation_text = format!("System: {}\n\n", system_prompt);
            let mut last_saved: Option<ChatMessageResponse> = None;

            for (step_idx, step) in steps.iter().enumerate() {
                tracing::info!(
                    item_id = %request.item_id,
                    step = step_idx + 1,
                    label = %step.display_label,
                    "Starting CLI analysis step"
                );

                // Save user message
                let user_msg_id = Uuid::new_v4().to_string();
                let now = chrono::Utc::now().naive_utc();
                {
                    let db = state.get_db().await?;
                    let user_msg = chat_message::ActiveModel {
                        id: Set(user_msg_id.clone()),
                        item_id: Set(request.item_id.clone()),
                        role: Set("user".to_string()),
                        content: Set(step.display_label.clone()),
                        created_at: Set(now),
                        input_tokens: Set(None),
                        output_tokens: Set(None),
                        model: Set(None),
                    };
                    user_msg.insert(&db).await.map_err(|e| {
                        tracing::error!(error = %e, item_id = %request.item_id, "Failed to save step user message");
                        CommandError::Internal {
                            message: e.to_string(),
                        }
                    })?;
                }

                // Emit user message event
                let user_chat_msg = ChatMessageResponse {
                    id: user_msg_id,
                    item_id: request.item_id.clone(),
                    role: "user".to_string(),
                    content: step.display_label.clone(),
                    created_at: now.to_string(),
                    input_tokens: None,
                    output_tokens: None,
                    model: None,
                };
                let _ = app.emit(
                    "ai-step-user-message",
                    serde_json::json!({
                        "item_id": &request.item_id,
                        "message": &user_chat_msg
                    }),
                );

                // Build CLI prompt with conversation so far
                conversation_text.push_str(&format!("User: {}\n\n", step.user_prompt));

                let binary = cli_tool.binary_name();
                let mut args = vec!["-p".to_string(), conversation_text.clone()];
                if let Some(ref m) = ctx.ai_model {
                    args.push("--model".to_string());
                    args.push(m.clone());
                }

                let _ = app.emit("ai-stream-start", &request.item_id);

                let output = tokio::process::Command::new(binary)
                    .current_dir(&cli_path)
                    .args(&args)
                    .output()
                    .await
                    .map_err(|e| {
                        tracing::error!(error = %e, item_id = %request.item_id, "Failed to run CLI for step");
                        CommandError::Internal {
                            message: format!("Failed to run {binary}: {e}"),
                        }
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::error!(stderr = %stderr, item_id = %request.item_id, "CLI returned error at step {}", step_idx + 1);
                    let _ = app.emit("ai-stream-end", &request.item_id);
                    return Err(CommandError::Internal {
                        message: format!("{binary} error: {stderr}"),
                    });
                }

                let response_content = String::from_utf8_lossy(&output.stdout).to_string();
                conversation_text.push_str(&format!("Assistant: {}\n\n", response_content));

                // Save assistant message
                let assistant_msg_id = Uuid::new_v4().to_string();
                let assistant_now = chrono::Utc::now().naive_utc();
                {
                    let db = state.get_db().await?;
                    let assistant_msg = chat_message::ActiveModel {
                        id: Set(assistant_msg_id.clone()),
                        item_id: Set(request.item_id.clone()),
                        role: Set("assistant".to_string()),
                        content: Set(response_content.clone()),
                        created_at: Set(assistant_now),
                        input_tokens: Set(None),
                        output_tokens: Set(None),
                        model: Set(ctx.ai_model.clone()),
                    };
                    assistant_msg.insert(&db).await.map_err(|e| {
                        tracing::error!(error = %e, item_id = %request.item_id, "Failed to save step assistant message");
                        CommandError::Internal {
                            message: e.to_string(),
                        }
                    })?;
                }

                let _ = app.emit("ai-stream-end", &request.item_id);

                let assistant_chat_msg = ChatMessageResponse {
                    id: assistant_msg_id,
                    item_id: request.item_id.clone(),
                    role: "assistant".to_string(),
                    content: response_content,
                    created_at: assistant_now.to_string(),
                    input_tokens: None,
                    output_tokens: None,
                    model: ctx.ai_model.clone(),
                };
                let _ = app.emit(
                    "ai-step-assistant-message",
                    serde_json::json!({
                        "item_id": &request.item_id,
                        "message": &assistant_chat_msg
                    }),
                );

                last_saved = Some(assistant_chat_msg);
            }

            let _ = app.emit("ai-analysis-complete", &request.item_id);

            Ok(last_saved.unwrap_or(ChatMessageResponse {
                id: String::new(),
                item_id: request.item_id.clone(),
                role: "assistant".to_string(),
                content: String::new(),
                created_at: chrono::Utc::now().naive_utc().to_string(),
                input_tokens: None,
                output_tokens: None,
                model: None,
            }))
        } else {
            // Single-step CLI for DraftResponse
            let response_content = ossue_core::services::provider::analyze_with_cli(
                &cli_tool,
                None,
                &action_type,
                &item_context,
                item_context.pr_diff.as_deref(),
                &cli_path,
                ctx.ai_model.as_deref(),
            )
            .await
            .map_err(|e| {
                tracing::error!(error = %e, item_id = %request.item_id, "CLI analysis failed");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;

            let db = state.get_db().await?;
            let now = chrono::Utc::now().naive_utc();

            let user_msg = chat_message::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                item_id: Set(request.item_id.clone()),
                role: Set("user".to_string()),
                content: Set(display_label.clone()),
                created_at: Set(now),
                input_tokens: Set(None),
                output_tokens: Set(None),
                model: Set(None),
            };
            user_msg.insert(&db).await.map_err(|e| {
                tracing::error!(error = %e, item_id = %request.item_id, "Failed to save user message");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;

            let assistant_msg = chat_message::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                item_id: Set(request.item_id.clone()),
                role: Set("assistant".to_string()),
                content: Set(response_content.clone()),
                created_at: Set(chrono::Utc::now().naive_utc()),
                input_tokens: Set(None),
                output_tokens: Set(None),
                model: Set(ctx.ai_model),
            };
            let saved = assistant_msg.insert(&db).await.map_err(|e| {
                tracing::error!(error = %e, item_id = %request.item_id, "Failed to save assistant message");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;

            let _ = app.emit("ai-stream-end", &request.item_id);

            Ok(ChatMessageResponse {
                id: saved.id,
                item_id: saved.item_id,
                role: saved.role,
                content: saved.content,
                created_at: saved.created_at.to_string(),
                input_tokens: saved.input_tokens,
                output_tokens: saved.output_tokens,
                model: saved.model,
            })
        }
    } else {
        // API mode: multi-step analysis flow
        let system_prompt = if action_type == ActionType::Analyze {
            ContextService::build_multi_step_system_prompt(&ctx.item.item_type)
        } else {
            ContextService::build_system_prompt(&action_type, &ctx.item.item_type)
        };

        let api_key_value = ctx.api_key_value.ok_or_else(|| {
            tracing::error!(item_id = %request.item_id, "AI API key not configured");
            CommandError::AiNotConfigured
        })?;

        let service =
            AiApiService::new_with_system(api_key_value, ctx.ai_model.clone(), system_prompt);
        let model_name = service.model().to_string();

        if action_type == ActionType::Analyze {
            // Multi-step analysis
            let steps = ContextService::build_analysis_steps(
                &ctx.item.item_type,
                &item_context,
                item_context.pr_diff.as_deref(),
            );

            // Initialize conversation with truncated chat history
            let mut api_conversation: Vec<ApiMessage> = chat_history
                .iter()
                .map(|m| ApiMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect();
            let api_conversation_base = truncate_chat_history(api_conversation);
            api_conversation = api_conversation_base;

            let mut last_saved: Option<ChatMessageResponse> = None;

            for (step_idx, step) in steps.iter().enumerate() {
                tracing::info!(
                    item_id = %request.item_id,
                    step = step_idx + 1,
                    label = %step.display_label,
                    "Starting analysis step"
                );

                // a. Save user message to DB (display_label as content)
                let user_msg_id = Uuid::new_v4().to_string();
                let now = chrono::Utc::now().naive_utc();
                {
                    let db = state.get_db().await?;
                    let user_msg = chat_message::ActiveModel {
                        id: Set(user_msg_id.clone()),
                        item_id: Set(request.item_id.clone()),
                        role: Set("user".to_string()),
                        content: Set(step.display_label.clone()),
                        created_at: Set(now),
                        input_tokens: Set(None),
                        output_tokens: Set(None),
                        model: Set(None),
                    };
                    user_msg.insert(&db).await.map_err(|e| {
                        tracing::error!(error = %e, item_id = %request.item_id, "Failed to save step user message");
                        CommandError::Internal {
                            message: e.to_string(),
                        }
                    })?;
                }

                // b. Emit user message event
                let user_chat_msg = ChatMessageResponse {
                    id: user_msg_id,
                    item_id: request.item_id.clone(),
                    role: "user".to_string(),
                    content: step.display_label.clone(),
                    created_at: now.to_string(),
                    input_tokens: None,
                    output_tokens: None,
                    model: None,
                };
                let _ = app.emit(
                    "ai-step-user-message",
                    serde_json::json!({
                        "item_id": &request.item_id,
                        "message": &user_chat_msg
                    }),
                );

                // c. Add step's user_prompt (NOT display_label) to api_conversation
                api_conversation.push(ApiMessage {
                    role: "user".to_string(),
                    content: step.user_prompt.clone(),
                });

                // d. Emit stream-start
                let _ = app.emit("ai-stream-start", &request.item_id);

                // e. Call stream_llm_response
                let (response_content, input_tokens, output_tokens) =
                    stream_llm_response(&app, &request.item_id, &service, &api_conversation)
                        .await?;

                // f. Save assistant message to DB
                let assistant_msg_id = Uuid::new_v4().to_string();
                let assistant_now = chrono::Utc::now().naive_utc();
                {
                    let db = state.get_db().await?;
                    let assistant_msg = chat_message::ActiveModel {
                        id: Set(assistant_msg_id.clone()),
                        item_id: Set(request.item_id.clone()),
                        role: Set("assistant".to_string()),
                        content: Set(response_content.clone()),
                        created_at: Set(assistant_now),
                        input_tokens: Set(input_tokens),
                        output_tokens: Set(output_tokens),
                        model: Set(Some(model_name.clone())),
                    };
                    assistant_msg.insert(&db).await.map_err(|e| {
                        tracing::error!(error = %e, item_id = %request.item_id, "Failed to save step assistant message");
                        CommandError::Internal {
                            message: e.to_string(),
                        }
                    })?;
                }

                // g. Emit stream-end (resets streaming content for next step)
                let _ = app.emit("ai-stream-end", &request.item_id);

                // h. Emit assistant message event
                let assistant_chat_msg = ChatMessageResponse {
                    id: assistant_msg_id,
                    item_id: request.item_id.clone(),
                    role: "assistant".to_string(),
                    content: response_content.clone(),
                    created_at: assistant_now.to_string(),
                    input_tokens,
                    output_tokens,
                    model: Some(model_name.clone()),
                };
                let _ = app.emit(
                    "ai-step-assistant-message",
                    serde_json::json!({
                        "item_id": &request.item_id,
                        "message": &assistant_chat_msg
                    }),
                );

                // i. Add assistant response to api_conversation
                api_conversation.push(ApiMessage {
                    role: "assistant".to_string(),
                    content: response_content,
                });

                last_saved = Some(assistant_chat_msg);
            }

            // Emit analysis-complete
            let _ = app.emit("ai-analysis-complete", &request.item_id);

            Ok(last_saved.unwrap_or(ChatMessageResponse {
                id: String::new(),
                item_id: request.item_id.clone(),
                role: "assistant".to_string(),
                content: String::new(),
                created_at: chrono::Utc::now().naive_utc().to_string(),
                input_tokens: None,
                output_tokens: None,
                model: None,
            }))
        } else {
            // Single-step for DraftResponse (unchanged logic)
            let mut api_messages: Vec<ApiMessage> = chat_history
                .iter()
                .map(|m| ApiMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect();

            api_messages.push(ApiMessage {
                role: "user".to_string(),
                content: action_prompt.clone(),
            });

            let api_messages = truncate_chat_history(api_messages);

            // Save the user message first
            {
                let db = state.get_db().await?;
                let user_msg = chat_message::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    item_id: Set(request.item_id.clone()),
                    role: Set("user".to_string()),
                    content: Set(display_label),
                    created_at: Set(chrono::Utc::now().naive_utc()),
                    input_tokens: Set(None),
                    output_tokens: Set(None),
                    model: Set(None),
                };
                user_msg.insert(&db).await.map_err(|e| {
                    tracing::error!(error = %e, item_id = %request.item_id, "Failed to save user message");
                    CommandError::Internal {
                        message: e.to_string(),
                    }
                })?;
            }

            let _ = app.emit(
                "ai-analysis-progress",
                serde_json::json!({
                    "item_id": &request.item_id,
                    "status": "Waiting for AI..."
                }),
            );
            let _ = app.emit("ai-stream-start", &request.item_id);

            let (response_content, input_tokens, output_tokens) =
                stream_llm_response(&app, &request.item_id, &service, &api_messages).await?;
            let _ = app.emit("ai-stream-end", &request.item_id);

            let db = state.get_db().await?;
            let assistant_msg = chat_message::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                item_id: Set(request.item_id.clone()),
                role: Set("assistant".to_string()),
                content: Set(response_content.clone()),
                created_at: Set(chrono::Utc::now().naive_utc()),
                input_tokens: Set(input_tokens),
                output_tokens: Set(output_tokens),
                model: Set(Some(model_name)),
            };
            let saved = assistant_msg.insert(&db).await.map_err(|e| {
                tracing::error!(error = %e, item_id = %request.item_id, "Failed to save assistant message");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;

            Ok(ChatMessageResponse {
                id: saved.id,
                item_id: saved.item_id,
                role: saved.role,
                content: saved.content,
                created_at: saved.created_at.to_string(),
                input_tokens: saved.input_tokens,
                output_tokens: saved.output_tokens,
                model: saved.model,
            })
        }
    };

    // Phase 5: Cleanup worktree (always, success or error)
    if let Some(ref wt) = worktree {
        let wt_clone = wt.clone();
        tokio::task::spawn_blocking(move || {
            RepoManager::cleanup_worktree(&wt_clone);
        })
        .await
        .ok();
    }

    result
}

#[tauri::command]
pub async fn clear_chat(state: State<'_, AppState>, item_id: String) -> Result<(), CommandError> {
    tracing::debug!(item_id = %item_id, "Clearing chat history");
    let db = state.get_db().await?;

    chat_message::Entity::delete_many()
        .filter(chat_message::Column::ItemId.eq(&item_id))
        .exec(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, item_id = %item_id, "Failed to clear chat history");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Streaming helper
// ---------------------------------------------------------------------------

/// Stream an LLM response via SSE, emitting chunks to the frontend.
/// Returns (full_response, input_tokens, output_tokens).
async fn stream_llm_response(
    app: &tauri::AppHandle,
    item_id: &str,
    service: &AiApiService,
    api_messages: &[ApiMessage],
) -> Result<(String, Option<i32>, Option<i32>), CommandError> {
    match service.send_message_streaming(api_messages).await {
        Ok(response) => {
            use futures_util::StreamExt;
            let mut stream = response.bytes_stream();
            let mut full_response = String::new();
            let mut buffer = String::new();
            let mut input_tokens: Option<i32> = None;
            let mut output_tokens: Option<i32> = None;

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        while let Some(pos) = buffer.find("\n\n") {
                            let event_block = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            for line in event_block.lines() {
                                if let Some(data) = line.strip_prefix("data: ") {
                                    if data == "[DONE]" {
                                        continue;
                                    }
                                    if let Ok(event) = serde_json::from_str::<
                                        ossue_core::services::ai_api::StreamEvent,
                                    >(data)
                                    {
                                        match event {
                                            ossue_core::services::ai_api::StreamEvent::MessageStart { message } => {
                                                input_tokens = message.usage.input_tokens.map(|v| v as i32);
                                            }
                                            ossue_core::services::ai_api::StreamEvent::ContentBlockDelta {
                                                delta,
                                                ..
                                            } => {
                                                if let Some(text) = delta.text {
                                                    full_response.push_str(&text);
                                                    let _ = app.emit(
                                                        "ai-stream-chunk",
                                                        serde_json::json!({
                                                            "item_id": item_id,
                                                            "chunk": &text
                                                        }),
                                                    );
                                                }
                                            }
                                            ossue_core::services::ai_api::StreamEvent::MessageDelta { usage, .. } => {
                                                output_tokens = usage.output_tokens.map(|v| v as i32);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, item_id = %item_id, "Stream chunk error");
                        return Err(CommandError::PlatformApi {
                            message: format!("Stream error: {e}"),
                        });
                    }
                }
            }

            tracing::info!(item_id = %item_id, response_len = full_response.len(), "AI streaming completed");
            Ok((full_response, input_tokens, output_tokens))
        }
        Err(e) => {
            // Fall back to non-streaming
            tracing::warn!(item_id = %item_id, error = %e, "Streaming failed, falling back to non-streaming");
            let (content, usage) = service.send_message(api_messages).await.map_err(|e| {
                tracing::error!(error = %e, item_id = %item_id, "Non-streaming AI request also failed");
                CommandError::Internal {
                    message: e.to_string(),
                }
            })?;
            Ok((
                content,
                usage.input_tokens.map(|v| v as i32),
                usage.output_tokens.map(|v| v as i32),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

const MAX_CHAT_HISTORY_CHARS: usize = 100_000;
const MAX_CHAT_MESSAGES: usize = 30;

/// Truncate chat history to stay within token budget.
/// Keeps the first message (original analysis prompt) and the most recent
/// messages within the character and message count budgets.
/// Preserves user/assistant alternation.
fn truncate_chat_history(messages: Vec<ApiMessage>) -> Vec<ApiMessage> {
    if messages.len() <= MAX_CHAT_MESSAGES {
        let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
        if total_chars <= MAX_CHAT_HISTORY_CHARS {
            return messages;
        }
    }

    if messages.is_empty() {
        return messages;
    }

    // Always keep the first message (original analysis context)
    let first = messages[0].clone();
    let rest = &messages[1..];

    // Take most recent messages that fit within budget
    let mut kept: Vec<ApiMessage> = Vec::new();
    let mut char_budget = MAX_CHAT_HISTORY_CHARS.saturating_sub(first.content.len());
    let msg_budget = MAX_CHAT_MESSAGES.saturating_sub(1);

    for msg in rest.iter().rev() {
        if kept.len() >= msg_budget {
            break;
        }
        if msg.content.len() > char_budget {
            break;
        }
        char_budget -= msg.content.len();
        kept.push(msg.clone());
    }

    kept.reverse();

    // Ensure we start with a user message after the first message
    // (to maintain valid alternation)
    if let Some(first_kept) = kept.first() {
        if first_kept.role == "assistant" {
            kept.remove(0);
        }
    }

    let original_count = messages.len();
    let mut result = vec![first];
    result.append(&mut kept);

    if result.len() < original_count {
        tracing::info!(
            original = original_count,
            truncated_to = result.len(),
            "Truncated chat history to fit within budget"
        );
    }

    result
}

/// Helper to get project token (direct or via connector).
/// Delegates to [`ossue_core::services::auth::get_project_token`].
async fn get_project_token(
    project: &ossue_core::models::project::Model,
    db: &DatabaseConnection,
) -> Result<String, CommandError> {
    ossue_core::services::auth::get_project_token(db, project)
        .await
        .map_err(|e| CommandError::Internal {
            message: e.to_string(),
        })
}

async fn get_setting(db: &DatabaseConnection, key: &str) -> Option<String> {
    settings_model::Entity::find_by_id(key)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|s| s.value)
}

/// Helper to get GitLab base URL.
/// Delegates to [`ossue_core::services::auth::get_project_base_url`].
async fn get_gitlab_base_url(
    project: &ossue_core::models::project::Model,
    db: &DatabaseConnection,
) -> Option<String> {
    ossue_core::services::auth::get_project_base_url(db, project).await
}

#[tauri::command]
pub async fn get_analyzed_item_ids(
    state: State<'_, AppState>,
) -> Result<Vec<String>, CommandError> {
    let db = state.get_db().await?;

    use ossue_core::models::{chat_message, item};
    use sea_orm::{QuerySelect, RelationTrait};

    let ids: Vec<String> = chat_message::Entity::find()
        .select_only()
        .column(chat_message::Column::ItemId)
        .distinct()
        .join(
            sea_orm::JoinType::InnerJoin,
            chat_message::Relation::Item.def(),
        )
        .filter(item::Column::IsDeleted.eq(false))
        .into_tuple::<String>()
        .all(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to query analyzed item IDs");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?;

    Ok(ids)
}

#[tauri::command]
pub async fn post_item_comment(
    state: State<'_, AppState>,
    item_id: String,
    comment: String,
) -> Result<(), CommandError> {
    let db = state.get_db().await?;

    // 1. Load item
    let item = ossue_core::models::item::Entity::find_by_id(&item_id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, item_id = %item_id, "Failed to query item");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Item".to_string(),
            id: item_id.clone(),
        })?;

    let type_data = item.parse_type_data().map_err(|e| CommandError::Internal {
        message: e.to_string(),
    })?;
    let external_id = type_data
        .external_id()
        .ok_or_else(|| CommandError::Internal {
            message: "Item has no external ID".to_string(),
        })?;

    // 2. Load project
    let project = ossue_core::models::project::Entity::find_by_id(&item.project_id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %item.project_id, "Failed to query project");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Project".to_string(),
            id: item.project_id.clone(),
        })?;

    let token = get_project_token(&project, &db).await?;
    let base_url = ossue_core::services::auth::get_project_base_url(&db, &project).await;

    match project.platform {
        ossue_core::enums::Platform::GitHub => {
            let client = ossue_core::services::github::GitHubClient::with_base_url(token, base_url);
            client
                .post_comment(&project.owner, &project.name, external_id, &comment)
                .await
                .map_err(|e| CommandError::PlatformApi {
                    message: e.to_string(),
                })?;
        }
        ossue_core::enums::Platform::GitLab => {
            let client = ossue_core::services::gitlab::GitLabClient::new(token, base_url);
            let gitlab_project_id =
                project
                    .external_project_id
                    .ok_or_else(|| CommandError::Internal {
                        message: "GitLab project has no external project ID".to_string(),
                    })?;
            client
                .post_comment(
                    gitlab_project_id,
                    external_id,
                    &item.item_type.to_string(),
                    &comment,
                )
                .await
                .map_err(|e| CommandError::PlatformApi {
                    message: e.to_string(),
                })?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn merge_pull_request(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<(), CommandError> {
    let db = state.get_db().await?;

    // 1. Load item
    let item = ossue_core::models::item::Entity::find_by_id(&item_id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, item_id = %item_id, "Failed to query item");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Item".to_string(),
            id: item_id.clone(),
        })?;

    // Verify it's a PR
    if item.item_type != ossue_core::enums::ItemType::PullRequest {
        return Err(CommandError::Internal {
            message: "Item is not a pull request".to_string(),
        });
    }

    let type_data = item.parse_type_data().map_err(|e| CommandError::Internal {
        message: e.to_string(),
    })?;
    let external_id = type_data
        .external_id()
        .ok_or_else(|| CommandError::Internal {
            message: "Item has no external ID".to_string(),
        })?;

    // 2. Load project
    let project = ossue_core::models::project::Entity::find_by_id(&item.project_id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %item.project_id, "Failed to query project");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Project".to_string(),
            id: item.project_id.clone(),
        })?;

    let token = get_project_token(&project, &db).await?;
    let base_url = ossue_core::services::auth::get_project_base_url(&db, &project).await;

    match project.platform {
        ossue_core::enums::Platform::GitHub => {
            let client = ossue_core::services::github::GitHubClient::with_base_url(token, base_url);
            client
                .merge_pull_request(&project.owner, &project.name, external_id)
                .await
                .map_err(|e| CommandError::PlatformApi {
                    message: e.to_string(),
                })?;
        }
        ossue_core::enums::Platform::GitLab => {
            let client = ossue_core::services::gitlab::GitLabClient::new(token, base_url);
            let gitlab_project_id =
                project
                    .external_project_id
                    .ok_or_else(|| CommandError::Internal {
                        message: "GitLab project has no external project ID".to_string(),
                    })?;
            client
                .merge_merge_request(gitlab_project_id, external_id)
                .await
                .map_err(|e| CommandError::PlatformApi {
                    message: e.to_string(),
                })?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn close_item(state: State<'_, AppState>, item_id: String) -> Result<(), CommandError> {
    let db = state.get_db().await?;

    // 1. Load item
    let item = ossue_core::models::item::Entity::find_by_id(&item_id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, item_id = %item_id, "Failed to query item");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Item".to_string(),
            id: item_id.clone(),
        })?;

    let type_data = item.parse_type_data().map_err(|e| CommandError::Internal {
        message: e.to_string(),
    })?;
    let external_id = type_data
        .external_id()
        .ok_or_else(|| CommandError::Internal {
            message: "Item has no external ID".to_string(),
        })?;

    // 2. Load project
    let project = ossue_core::models::project::Entity::find_by_id(&item.project_id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = %item.project_id, "Failed to query project");
            CommandError::Internal {
                message: e.to_string(),
            }
        })?
        .ok_or_else(|| CommandError::NotFound {
            entity: "Project".to_string(),
            id: item.project_id.clone(),
        })?;

    let token = get_project_token(&project, &db).await?;
    let base_url = ossue_core::services::auth::get_project_base_url(&db, &project).await;

    match project.platform {
        ossue_core::enums::Platform::GitHub => {
            let client = ossue_core::services::github::GitHubClient::with_base_url(token, base_url);
            client
                .close_issue(&project.owner, &project.name, external_id)
                .await
                .map_err(|e| CommandError::PlatformApi {
                    message: e.to_string(),
                })?;
        }
        ossue_core::enums::Platform::GitLab => {
            let client = ossue_core::services::gitlab::GitLabClient::new(token, base_url);
            let gitlab_project_id =
                project
                    .external_project_id
                    .ok_or_else(|| CommandError::Internal {
                        message: "GitLab project has no external project ID".to_string(),
                    })?;
            client
                .close_issue(gitlab_project_id, external_id)
                .await
                .map_err(|e| CommandError::PlatformApi {
                    message: e.to_string(),
                })?;
        }
    }

    Ok(())
}
