use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::reload;

pub type LogReloadHandle = reload::Handle<LevelFilter, tracing_subscriber::Registry>;

#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntriesResponse {
    pub entries: Vec<LogEntry>,
    pub total: usize,
    pub has_more: bool,
}

/// Initialize logging with a daily rolling file appender.
/// Returns the reload handle and the log directory path.
pub fn init_logging() -> (LogReloadHandle, PathBuf) {
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| {
            tracing::warn!("Could not determine data directory, falling back to current directory");
            PathBuf::from(".")
        })
        .join(crate::APP_DIR_NAME)
        .join("logs");

    let _ = std::fs::create_dir_all(&log_dir);
    tracing::debug!(path = %log_dir.display(), "Log directory initialized");

    // Daily rolling file appender
    let file_appender = tracing_appender::rolling::daily(&log_dir, "app.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard so it lives for the program's lifetime
    std::mem::forget(_guard);

    // Reload layer for hot-reloading log level
    let (filter, reload_handle) = reload::Layer::new(LevelFilter::WARN);

    // JSON format layer
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_timer(tracing_subscriber::fmt::time::SystemTime)
        .with_target(true)
        .with_writer(non_blocking);

    // Stdout layer for development (debug builds only)
    #[cfg(debug_assertions)]
    let stdout_layer = Some(tracing_subscriber::fmt::layer().with_target(true).compact());
    #[cfg(not(debug_assertions))]
    let stdout_layer: Option<tracing_subscriber::fmt::Layer<_>> = None;

    // Build subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(json_layer)
        .with(stdout_layer)
        .init();

    (reload_handle, log_dir)
}

/// Clean up old log files (older than 14 days).
pub fn cleanup_old_logs(log_dir: &std::path::Path) {
    if !log_dir.exists() {
        return;
    }

    let cutoff = chrono::Utc::now() - chrono::Duration::days(14);
    let cutoff_system = std::time::SystemTime::UNIX_EPOCH
        + std::time::Duration::from_secs(cutoff.timestamp() as u64);

    let mut removed_count = 0u32;
    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("app.log") {
                continue;
            }
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff_system {
                        let _ = std::fs::remove_file(entry.path());
                        removed_count += 1;
                    }
                }
            }
        }
    }
    if removed_count > 0 {
        tracing::info!(count = removed_count, "Cleaned up old log files");
    }
}

/// Read and parse log entries from log files.
pub fn read_log_entries(
    log_dir: &std::path::Path,
    level_filter: Option<&str>,
    text_filter: Option<&str>,
    limit: usize,
    offset: usize,
) -> LogEntriesResponse {
    let mut all_entries: Vec<LogEntry> = Vec::new();

    if log_dir.exists() {
        let mut log_files: Vec<_> = std::fs::read_dir(log_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("app.log") {
                    Some(entry.path())
                } else {
                    None
                }
            })
            .collect();

        // Sort files newest first
        log_files.sort_by(|a, b| b.cmp(a));

        for file_path in log_files {
            let file_date = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|name| name.strip_prefix("app.log."))
                .and_then(|date_str| NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok());

            let content = std::fs::read_to_string(&file_path).unwrap_or_default();
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                    let level = json
                        .get("level")
                        .and_then(|v| v.as_str())
                        .unwrap_or("INFO")
                        .to_string();
                    let raw_ts = json
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let timestamp = parse_uptime_to_iso(&raw_ts, file_date).unwrap_or(raw_ts);
                    let target = json
                        .get("target")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let fields_obj = json.get("fields");
                    let message = fields_obj
                        .and_then(|f| f.get("message"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let mut fields = HashMap::new();
                    if let Some(obj) = fields_obj.and_then(|f| f.as_object()) {
                        for (k, v) in obj {
                            if k == "message" {
                                continue;
                            }
                            let val = match v {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            fields.insert(k.clone(), val);
                        }
                    }

                    all_entries.push(LogEntry {
                        timestamp,
                        level,
                        target,
                        message,
                        fields,
                    });
                }
            }
        }
    }

    // Sort newest first
    all_entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Apply level filter
    if let Some(lf) = level_filter {
        let filter_level = lf.to_uppercase();
        let min_level = match filter_level.as_str() {
            "TRACE" => 0,
            "DEBUG" => 1,
            "INFO" => 2,
            "WARN" => 3,
            "ERROR" => 4,
            _ => 0,
        };

        all_entries.retain(|e| {
            let entry_level = match e.level.to_uppercase().as_str() {
                "TRACE" => 0,
                "DEBUG" => 1,
                "INFO" => 2,
                "WARN" => 3,
                "ERROR" => 4,
                _ => 0,
            };
            entry_level >= min_level
        });
    }

    // Apply text filter
    if let Some(tf) = text_filter {
        let tf_lower = tf.to_lowercase();
        all_entries.retain(|e| {
            e.message.to_lowercase().contains(&tf_lower)
                || e.target.to_lowercase().contains(&tf_lower)
        });
    }

    let total = all_entries.len();
    let entries: Vec<LogEntry> = all_entries.into_iter().skip(offset).take(limit).collect();
    let has_more = offset + entries.len() < total;

    LogEntriesResponse {
        entries,
        total,
        has_more,
    }
}

/// Convert uptime timestamp like "   1.818375625s" to an ISO string
/// using the log file's date. Returns `None` if already ISO or unparseable.
pub fn parse_uptime_to_iso(raw: &str, file_date: Option<NaiveDate>) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('s');
    let secs: f64 = trimmed.parse().ok()?;
    let file_date = file_date?;
    let total_secs = secs as u32;
    let nanos = ((secs - total_secs as f64) * 1_000_000_000.0) as u32;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    let time = NaiveTime::from_hms_nano_opt(hours, minutes, seconds, nanos)?;
    let dt = NaiveDateTime::new(file_date, time);
    Some(dt.format("%Y-%m-%dT%H:%M:%S").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // -----------------------------------------------------------------------
    // parse_uptime_to_iso
    // -----------------------------------------------------------------------

    #[rstest]
    #[case("1.0s", Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()), Some("2024-01-15T00:00:01"))]
    #[case("3661.5s", Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()), Some("2024-01-15T01:01:01"))]
    #[case("  1.0s", Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()), Some("2024-01-15T00:00:01"))]
    #[case("1.0", Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()), Some("2024-01-15T00:00:01"))]
    #[case("notanumber", Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()), None)]
    #[case("1.0s", None, None)]
    fn test_parse_uptime_to_iso(
        #[case] raw: &str,
        #[case] file_date: Option<NaiveDate>,
        #[case] expected: Option<&str>,
    ) {
        let result = parse_uptime_to_iso(raw, file_date);
        assert_eq!(result.as_deref(), expected);
    }

    // -----------------------------------------------------------------------
    // read_log_entries
    // -----------------------------------------------------------------------

    fn make_log_line(level: &str, target: &str, message: &str) -> String {
        serde_json::json!({
            "level": level,
            "timestamp": "1.0s",
            "target": target,
            "fields": { "message": message }
        })
        .to_string()
    }

    #[test]
    fn read_log_entries_empty_dir() {
        let tree = tree_fs::TreeBuilder::default().create().unwrap();
        let resp = read_log_entries(&tree.root, None, None, 100, 0);
        assert!(resp.entries.is_empty());
        assert_eq!(resp.total, 0);
        assert!(!resp.has_more);
    }

    #[test]
    fn read_log_entries_parses_json_lines() {
        let line1 = make_log_line("WARN", "mymod", "test warn");
        let line2 = make_log_line("ERROR", "mymod", "test error");
        let content = format!("{line1}\n{line2}\n");

        let tree = tree_fs::TreeBuilder::default()
            .add_file("app.log.2024-01-15", &content)
            .create()
            .unwrap();

        let resp = read_log_entries(&tree.root, None, None, 100, 0);
        assert_eq!(resp.entries.len(), 2);
        assert_eq!(resp.total, 2);
        assert!(!resp.has_more);

        for entry in &resp.entries {
            assert!(!entry.level.is_empty());
            assert!(!entry.message.is_empty());
        }
    }

    #[test]
    fn read_log_entries_level_filter() {
        let info_line = make_log_line("INFO", "mod", "info msg");
        let warn_line = make_log_line("WARN", "mod", "warn msg");
        let error_line = make_log_line("ERROR", "mod", "error msg");
        let content = format!("{info_line}\n{warn_line}\n{error_line}\n");

        let tree = tree_fs::TreeBuilder::default()
            .add_file("app.log.2024-01-15", &content)
            .create()
            .unwrap();

        let resp = read_log_entries(&tree.root, Some("WARN"), None, 100, 0);
        assert_eq!(resp.total, 2);
        let levels: Vec<&str> = resp.entries.iter().map(|e| e.level.as_str()).collect();
        assert!(levels.contains(&"WARN"));
        assert!(levels.contains(&"ERROR"));
        assert!(!levels.contains(&"INFO"));
    }

    #[test]
    fn read_log_entries_text_filter() {
        let line1 = make_log_line("INFO", "http", "request started");
        let line2 = make_log_line("INFO", "db", "query executed");
        let content = format!("{line1}\n{line2}\n");

        let tree = tree_fs::TreeBuilder::default()
            .add_file("app.log.2024-01-15", &content)
            .create()
            .unwrap();

        // Search by message (case-insensitive)
        let resp = read_log_entries(&tree.root, None, Some("REQUEST"), 100, 0);
        assert_eq!(resp.total, 1);
        assert_eq!(resp.entries[0].message, "request started");

        // Search by target
        let resp = read_log_entries(&tree.root, None, Some("db"), 100, 0);
        assert_eq!(resp.total, 1);
        assert_eq!(resp.entries[0].target, "db");
    }

    #[test]
    fn read_log_entries_pagination() {
        let mut lines = Vec::new();
        for i in 0..5 {
            lines.push(make_log_line("INFO", "mod", &format!("msg {i}")));
        }
        let content = lines.join("\n") + "\n";

        let tree = tree_fs::TreeBuilder::default()
            .add_file("app.log.2024-01-15", &content)
            .create()
            .unwrap();

        // First page
        let resp = read_log_entries(&tree.root, None, None, 2, 0);
        assert_eq!(resp.entries.len(), 2);
        assert_eq!(resp.total, 5);
        assert!(resp.has_more);

        // Second page
        let resp = read_log_entries(&tree.root, None, None, 2, 2);
        assert_eq!(resp.entries.len(), 2);
        assert!(resp.has_more);

        // Last page
        let resp = read_log_entries(&tree.root, None, None, 2, 4);
        assert_eq!(resp.entries.len(), 1);
        assert!(!resp.has_more);
    }

    #[test]
    fn read_log_entries_ignores_non_json_lines() {
        let good_line = make_log_line("INFO", "mod", "good");
        let content = format!("this is not json\n{good_line}\nalso bad {{[\n");

        let tree = tree_fs::TreeBuilder::default()
            .add_file("app.log.2024-01-15", &content)
            .create()
            .unwrap();

        let resp = read_log_entries(&tree.root, None, None, 100, 0);
        assert_eq!(resp.total, 1);
        assert_eq!(resp.entries[0].message, "good");
    }

    #[test]
    fn read_log_entries_ignores_non_app_log_files() {
        let line = make_log_line("INFO", "mod", "should appear");
        let other = make_log_line("INFO", "mod", "should NOT appear");

        let tree = tree_fs::TreeBuilder::default()
            .add_file("app.log.2024-01-15", &line)
            .add_file("debug.log", &other)
            .create()
            .unwrap();

        let resp = read_log_entries(&tree.root, None, None, 100, 0);
        assert_eq!(resp.total, 1);
        assert_eq!(resp.entries[0].message, "should appear");
    }

    // -----------------------------------------------------------------------
    // cleanup_old_logs
    // -----------------------------------------------------------------------

    #[test]
    fn cleanup_old_logs_keeps_recent_files() {
        let tree = tree_fs::TreeBuilder::default()
            .add_file("app.log.2024-01-15", "recent log")
            .create()
            .unwrap();

        // The file was just created, so its mtime is now (< 14 days old)
        cleanup_old_logs(&tree.root);

        // File should still exist
        assert!(tree.root.join("app.log.2024-01-15").exists());
    }

    #[test]
    fn cleanup_old_logs_nonexistent_dir() {
        let dir = std::path::Path::new("/tmp/nonexistent-log-dir-test-12345");
        // Should not panic
        cleanup_old_logs(dir);
    }

    #[test]
    fn cleanup_old_logs_ignores_non_app_log_files() {
        let tree = tree_fs::TreeBuilder::default()
            .add_file("debug.log", "not an app log")
            .create()
            .unwrap();

        cleanup_old_logs(&tree.root);

        // Non-app.log files should be untouched
        assert!(tree.root.join("debug.log").exists());
    }
}
