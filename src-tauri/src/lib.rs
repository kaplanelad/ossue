use ossue_core::logging::LogReloadHandle;
use sea_orm::EntityTrait;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tokio::sync::Mutex;
use tracing_subscriber::filter::LevelFilter;

use sea_orm::DatabaseConnection;

mod commands;

pub struct AppState {
    pub db: Arc<tokio::sync::RwLock<Option<DatabaseConnection>>>,
    pub log_reload_handle: LogReloadHandle,
    pub log_dir: PathBuf,
    pub syncing_projects: Arc<Mutex<HashSet<String>>>,
    pub retry_handles: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
    pub repo_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    pub repo_manager: Arc<ossue_core::services::repo_manager::RepoManager>,
}

impl AppState {
    pub async fn get_db(&self) -> Result<DatabaseConnection, commands::error::CommandError> {
        self.db
            .read()
            .await
            .clone()
            .ok_or(commands::error::CommandError::DatabaseNotReady)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    fix_path_env();

    let (reload_handle, log_dir) = ossue_core::logging::init_logging();

    // Capture panics to the log before the process exits
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());
        tracing::error!(panic.payload = %payload, panic.location = %location, "Application panicked");
    }));

    let log_dir_clone = log_dir.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            db: Arc::new(tokio::sync::RwLock::new(None)),
            log_reload_handle: reload_handle,
            log_dir: log_dir_clone,
            syncing_projects: Arc::new(Mutex::new(HashSet::new())),
            retry_handles: Arc::new(Mutex::new(HashMap::new())),
            repo_locks: Arc::new(Mutex::new(HashMap::new())),
            repo_manager: Arc::new(ossue_core::services::repo_manager::RepoManager::new()),
        })
        .setup(|app| {
            // System tray icon
            let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().unwrap())
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = init_db(&app_handle).await {
                    tracing::error!("Failed to initialize database: {e}");
                    let _ = app_handle.emit("db:init-error", e.to_string());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Auth
            commands::auth::get_auth_status,
            commands::auth::save_github_token,
            commands::auth::save_gitlab_token,
            commands::auth::disconnect_github,
            commands::auth::disconnect_gitlab,
            commands::auth::list_github_repos,
            commands::auth::list_gitlab_projects,
            // Connectors
            commands::connectors::list_connectors,
            commands::connectors::add_connector,
            commands::connectors::update_connector,
            commands::connectors::remove_connector,
            commands::connectors::list_connector_repos,
            // Repos
            commands::repos::list_projects,
            commands::repos::add_project,
            commands::repos::add_project_by_url,
            commands::repos::remove_project,
            commands::repos::prepare_repo,
            commands::repos::clear_repo_cache,
            commands::repos::toggle_project_sync,
            // Items
            commands::items::list_items,
            commands::items::get_item,
            commands::items::mark_item_read,
            commands::items::toggle_item_star,
            commands::items::delete_item,
            commands::items::list_dismissed_items,
            commands::items::restore_item,
            commands::items::clear_project_data,
            commands::items::sync_project_items,
            commands::items::full_sync_project_items,
            // AI
            commands::ai::get_chat_messages,
            commands::ai::send_chat_message,
            commands::ai::auto_analyze_item,
            commands::ai::analyze_item_action,
            commands::ai::clear_chat,
            commands::ai::get_analyzed_item_ids,
            // Project Notes
            commands::project_notes::list_project_notes,
            commands::project_notes::add_project_note,
            commands::project_notes::remove_project_note,
            // Draft Issues
            commands::draft_issues::list_draft_issues,
            commands::draft_issues::create_draft_issue,
            commands::draft_issues::update_draft_issue,
            commands::draft_issues::delete_draft_issue,
            commands::draft_issues::generate_issue_from_draft,
            commands::draft_issues::submit_draft_to_provider,
            commands::draft_issues::get_draft_issue_count,
            commands::draft_issues::toggle_draft_issue_star,
            // Settings
            commands::settings::get_settings,
            commands::settings::update_setting,
            commands::settings::delete_setting,
            commands::settings::is_onboarding_complete,
            commands::settings::get_app_paths,
            commands::settings::get_ai_settings,
            commands::settings::get_project_settings,
            commands::settings::update_project_setting,
            commands::settings::delete_project_setting,
            // Database
            commands::database::create_backup,
            commands::database::list_backups,
            commands::database::restore_backup,
            commands::database::reset_database,
            commands::database::delete_backup,
            // Logging
            commands::logging::get_log_level,
            commands::logging::set_log_level,
            commands::logging::get_log_entries,
            commands::logging::clear_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub async fn get_repo_lock(
    repo_locks: &Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    key: &str,
) -> Arc<tokio::sync::Mutex<()>> {
    let mut locks = repo_locks.lock().await;
    locks
        .entry(key.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// GUI apps launched outside a terminal (e.g., Finder/Spotlight on macOS,
/// desktop launchers on Linux) get a minimal PATH that doesn't include
/// directories like `~/.local/bin` or `~/.npm-global/bin` where tools such as
/// `claude` are installed.
///
/// Strategy:
/// 1. (Unix only) Source the user's login shell to obtain the full PATH.
/// 2. Append well-known directories that exist on disk but aren't already on
///    PATH. This covers shell mismatches (e.g. `$SHELL` is zsh but the user
///    actually uses fish), Windows GUI launches, and Linux desktop launchers.
fn fix_path_env() {
    use std::path::PathBuf;

    // On Unix: source the user's login shell to get the full PATH
    #[cfg(unix)]
    {
        use std::process::Command;
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        if let Ok(output) = Command::new(&shell)
            .args(["-l", "-c", "printf '%s' \"$PATH\""])
            .output()
        {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    if !path.is_empty() {
                        std::env::set_var("PATH", &path);
                    }
                }
            }
        }
    }

    // Append well-known directories that exist on disk but aren't on PATH.
    let (home, separator) = {
        #[cfg(unix)]
        {
            (std::env::var("HOME").unwrap_or_default(), ':')
        }
        #[cfg(windows)]
        {
            (std::env::var("USERPROFILE").unwrap_or_default(), ';')
        }
    };

    if !home.is_empty() {
        let mut extra: Vec<String> =
            vec![format!("{home}/.local/bin"), format!("{home}/.cargo/bin")];

        #[cfg(target_os = "macos")]
        extra.push("/opt/homebrew/bin".into());

        extra.push(if cfg!(windows) {
            format!("{home}\\AppData\\Local\\Programs\\claude-code\\bin")
        } else {
            "/usr/local/bin".into()
        });

        let current = std::env::var("PATH").unwrap_or_default();
        let existing: std::collections::HashSet<&str> = current.split(separator).collect();

        let missing: Vec<&str> = extra
            .iter()
            .map(String::as_str)
            .filter(|d| !existing.contains(d) && PathBuf::from(d).is_dir())
            .collect();

        if !missing.is_empty() {
            let sep = separator.to_string();
            std::env::set_var(
                "PATH",
                format!("{current}{separator}{}", missing.join(&sep)),
            );
        }
    }
}

async fn init_db(app: &tauri::AppHandle) -> Result<(), ossue_core::error::InitError> {
    let db = ossue_core::db::init_database().await?;

    // Restore persisted log level from settings
    let state = app.state::<AppState>();
    if let Ok(Some(setting)) = ossue_core::models::settings::Entity::find_by_id("log_level")
        .one(&db)
        .await
    {
        if let Ok(level) = setting.value.parse::<LevelFilter>() {
            let _ = state.log_reload_handle.modify(|f| *f = level);
            tracing::info!(level = %level, "Restored log level from settings");
        }
    }

    // Clean up old log files (older than 14 days)
    ossue_core::logging::cleanup_old_logs(&state.log_dir);

    // Reset any notes stuck in ai_processing from a previous crash (raw SQL
    // to avoid deserialization failures after removing the AiProcessing enum variant)
    {
        use sea_orm::ConnectionTrait;
        let result = db
            .execute(sea_orm::Statement::from_string(
                db.get_database_backend(),
                "UPDATE items SET type_data = json_set(type_data, '$.draft_status', 'draft') WHERE json_extract(type_data, '$.draft_status') = 'ai_processing' AND item_type = 'note'".to_string(),
            ))
            .await;
        match result {
            Ok(res) if res.rows_affected() > 0 => {
                tracing::info!(
                    count = res.rows_affected(),
                    "Reset stuck ai_processing notes to draft"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to reset stuck ai_processing notes");
            }
            _ => {}
        }
    }

    // Store in state
    *state.db.write().await = Some(db);

    tracing::info!("Database initialized successfully");

    // Spawn startup sync: emit events for all sync-enabled projects + clean up stale worktrees
    let app_handle = app.app_handle().clone();
    let db_arc = state.db.clone();
    tauri::async_runtime::spawn(async move {
        // Small delay to let the app UI initialize and set up event listeners
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        if let Some(db) = db_arc.read().await.clone() {
            use ossue_core::models::project;

            let projects = project::Entity::find().all(&db).await;

            if let Ok(projects) = projects {
                // Clean up stale worktrees from previous crashes
                for proj in &projects {
                    ossue_core::services::repo_manager::RepoManager::cleanup_stale_worktrees(
                        &proj.platform,
                        &proj.owner,
                        &proj.name,
                    );
                }
                tracing::info!("Stale worktree cleanup complete");

                // Start sync for enabled projects
                let sync_projects: Vec<_> = projects.iter().filter(|p| p.sync_enabled).collect();
                tracing::info!(
                    count = sync_projects.len(),
                    "Starting startup sync for enabled projects"
                );
                for proj in &sync_projects {
                    let _ = app_handle.emit("startup:sync", &proj.id);
                }
            }
        }
    });

    Ok(())
}
