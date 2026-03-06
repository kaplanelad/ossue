use serde::Serialize;
use tauri::AppHandle;

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub release_notes: String,
    pub published_at: String,
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    match check_for_update_inner(&app).await {
        Ok(info) => Ok(info),
        Err(e) => {
            tracing::warn!(error = %e, "Update check failed");
            Ok(None)
        }
    }
}

async fn check_for_update_inner(app: &AppHandle) -> Result<Option<UpdateInfo>, Box<dyn std::error::Error>> {
    let current_version = app.package_info().version.to_string();

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/repos/kaplanelad/ossue/releases/latest")
        .header("User-Agent", format!("ossue/{current_version}"))
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;

    let tag_name = body["tag_name"]
        .as_str()
        .ok_or("missing tag_name")?;

    let latest_str = tag_name.strip_prefix('v').unwrap_or(tag_name);
    let latest = semver::Version::parse(latest_str)?;
    let current = semver::Version::parse(&current_version)?;

    if latest <= current {
        return Ok(None);
    }

    Ok(Some(UpdateInfo {
        current_version,
        latest_version: latest_str.to_string(),
        release_url: body["html_url"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        release_notes: body["body"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        published_at: body["published_at"]
            .as_str()
            .unwrap_or("")
            .to_string(),
    }))
}
