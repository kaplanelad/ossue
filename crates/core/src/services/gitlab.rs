use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::issue_creator::{CreateIssueRequest, CreateIssueResponse, IssueCreator};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error("Failed to decode response")]
    Decode(#[from] serde_json::Error),

    #[error("{0}")]
    Api(String),
}

type Result<T> = std::result::Result<T, Error>;

const MAX_PAGES: u32 = 50;

#[derive(Debug, Clone)]
pub struct GitLabClient {
    token: String,
    client: reqwest::Client,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitLabProject {
    pub id: i64,
    pub name: String,
    pub path_with_namespace: String,
    pub web_url: String,
    pub description: Option<String>,
    pub namespace: GitLabNamespace,
    pub star_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitLabNamespace {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitLabIssue {
    pub iid: i32,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub web_url: String,
    pub author: GitLabAuthor,
    pub user_notes_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitLabAuthor {
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitLabMergeRequest {
    pub iid: i32,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub web_url: String,
    pub author: GitLabAuthor,
    pub user_notes_count: i32,
    pub source_branch: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitLabNote {
    pub id: i64,
    pub body: String,
    pub author: GitLabNoteAuthor,
    pub created_at: String,
    pub updated_at: String,
    pub system: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitLabNoteAuthor {
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitLabCommit {
    pub id: String,
    pub message: String,
    pub author_name: String,
    pub created_at: String,
}

impl GitLabClient {
    pub fn new(token: String, base_url: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("ossue")
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
        let resolved_url = base_url.unwrap_or_else(|| "https://gitlab.com".to_string());
        tracing::debug!(base_url = %resolved_url, "GitLab client initialized");
        Self {
            token,
            client,
            base_url: resolved_url,
        }
    }

    pub async fn get_project_id(&self, path_with_namespace: &str) -> Result<i64> {
        let encoded = urlencoding::encode(path_with_namespace);
        tracing::info!(path = %path_with_namespace, "Resolving GitLab project ID");
        let body = self
            .client
            .get(format!("{}/api/v4/projects/{}", self.base_url, encoded))
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, path = %path_with_namespace, "GitLab API network error resolving project ID");
                e
            })?
            .error_for_status()
            .inspect_err(|e| {
                tracing::error!(status = ?e.status(), path = %path_with_namespace, "GitLab API error resolving project ID");
            })?
            .text()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, path = %path_with_namespace, "Failed to read GitLab project response body");
                e
            })?;

        let project: GitLabProject = serde_json::from_str(&body)
            .map_err(|e| {
                tracing::error!(error = %e, path = %path_with_namespace, body_preview = %&body[..body.len().min(200)], "Failed to decode GitLab project response");
                e
            })?;

        tracing::info!(path = %path_with_namespace, project_id = project.id, "Resolved GitLab project ID");
        Ok(project.id)
    }

    pub async fn list_projects(&self) -> Result<Vec<GitLabProject>> {
        tracing::info!("Fetching GitLab projects");
        let mut all_projects = Vec::new();
        let mut page = 1;

        loop {
            let response = self
                .client
                .get(format!("{}/api/v4/projects", self.base_url))
                .header("PRIVATE-TOKEN", &self.token)
                .query(&[
                    ("membership", "true"),
                    ("per_page", "100"),
                    ("order_by", "updated_at"),
                    ("owned", "true"),
                    ("page", &page.to_string()),
                ])
                .send()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "GitLab API network error fetching projects");
                    e
                })?;

            let body = response
                .error_for_status()
                .inspect_err(|e| {
                    tracing::error!(status = ?e.status(), "GitLab API error fetching projects");
                })?
                .text()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to read GitLab projects response body");
                    e
                })?;

            let projects: Vec<GitLabProject> = serde_json::from_str(&body)
                .map_err(|e| {
                    tracing::error!(error = %e, body_preview = %&body[..body.len().min(200)], "Failed to decode GitLab projects response");
                    e
                })?;

            tracing::debug!(
                page = page,
                count = projects.len(),
                "Fetched GitLab projects page"
            );

            if projects.is_empty() {
                break;
            }
            all_projects.extend(projects);
            page += 1;
            if page > MAX_PAGES {
                tracing::warn!("Reached maximum pagination limit");
                break;
            }
        }

        tracing::info!(total = all_projects.len(), "Fetched all GitLab projects");
        Ok(all_projects)
    }

    pub async fn list_labels(&self, owner: &str, repo: &str) -> Result<Vec<String>> {
        let path = format!("{owner}/{repo}");
        tracing::info!(path = %path, "Fetching GitLab repository labels");

        let project_id = self.get_project_id(&path).await?;

        #[derive(Debug, Deserialize)]
        struct GitLabLabel {
            name: String,
        }

        let response = self
            .client
            .get(format!(
                "{}/api/v4/projects/{project_id}/labels",
                self.base_url
            ))
            .header("PRIVATE-TOKEN", &self.token)
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, path = %path, "GitLab API network error fetching labels");
                e
            })?;

        let body = response
            .error_for_status()
            .inspect_err(|e| {
                tracing::error!(status = ?e.status(), path = %path, "GitLab API error fetching labels");
            })?
            .text()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, path = %path, "Failed to read labels response body");
                e
            })?;

        let labels: Vec<GitLabLabel> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, path = %path, body_preview = %&body[..body.len().min(200)], "Failed to decode labels response");
            e
        })?;

        let names: Vec<String> = labels.into_iter().map(|l| l.name).collect();
        tracing::info!(path = %path, count = names.len(), "Fetched GitLab repository labels");
        Ok(names)
    }

    pub async fn list_issues(&self, project_id: i64) -> Result<Vec<GitLabIssue>> {
        tracing::info!("Fetching GitLab issues for project {}", project_id);
        let mut all_issues = Vec::new();
        let mut page = 1;

        loop {
            let response = self
                .client
                .get(format!(
                    "{}/api/v4/projects/{project_id}/issues",
                    self.base_url
                ))
                .header("PRIVATE-TOKEN", &self.token)
                .query(&[
                    ("state", "opened"),
                    ("per_page", "100"),
                    ("page", &page.to_string()),
                ])
                .send()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, project_id = project_id, "GitLab API network error fetching issues");
                    e
                })?;

            let body = response
                .error_for_status()
                .inspect_err(|e| {
                    tracing::error!(status = ?e.status(), project_id = project_id, "GitLab API error fetching issues");
                })?
                .text()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, project_id = project_id, "Failed to read GitLab issues response body");
                    e
                })?;

            let issues: Vec<GitLabIssue> = serde_json::from_str(&body)
                .map_err(|e| {
                    tracing::error!(error = %e, project_id = project_id, body_preview = %&body[..body.len().min(200)], "Failed to decode GitLab issues response");
                    e
                })?;

            tracing::debug!(
                page = page,
                count = issues.len(),
                "Fetched GitLab issues page"
            );

            if issues.is_empty() {
                break;
            }
            all_issues.extend(issues);
            page += 1;
            if page > MAX_PAGES {
                tracing::warn!("Reached maximum pagination limit");
                break;
            }
        }

        tracing::info!(total = all_issues.len(), "Fetched all GitLab issues");
        Ok(all_issues)
    }

    pub async fn list_merge_requests(&self, project_id: i64) -> Result<Vec<GitLabMergeRequest>> {
        tracing::info!(project_id = project_id, "Fetching GitLab merge requests");
        let mut all_mrs = Vec::new();
        let mut page = 1;

        loop {
            let response = self
                .client
                .get(format!(
                    "{}/api/v4/projects/{project_id}/merge_requests",
                    self.base_url
                ))
                .header("PRIVATE-TOKEN", &self.token)
                .query(&[
                    ("state", "opened"),
                    ("per_page", "100"),
                    ("page", &page.to_string()),
                ])
                .send()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, project_id = project_id, "GitLab API network error fetching merge requests");
                    e
                })?;

            let body = response
                .error_for_status()
                .inspect_err(|e| {
                    tracing::error!(status = ?e.status(), project_id = project_id, "GitLab API error fetching merge requests");
                })?
                .text()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, project_id = project_id, "Failed to read GitLab merge requests response body");
                    e
                })?;

            let mrs: Vec<GitLabMergeRequest> = serde_json::from_str(&body)
                .map_err(|e| {
                    tracing::error!(error = %e, project_id = project_id, body_preview = %&body[..body.len().min(200)], "Failed to decode GitLab merge requests response");
                    e
                })?;

            tracing::debug!(
                page = page,
                count = mrs.len(),
                "Fetched GitLab merge requests page"
            );

            if mrs.is_empty() {
                break;
            }
            all_mrs.extend(mrs);
            page += 1;
            if page > MAX_PAGES {
                tracing::warn!("Reached maximum pagination limit");
                break;
            }
        }

        tracing::info!(total = all_mrs.len(), "Fetched all GitLab merge requests");
        Ok(all_mrs)
    }

    /// Fetch a single page of issues. Returns (items, has_more_pages).
    pub async fn fetch_issues_page(
        &self,
        project_id: i64,
        page: u32,
        updated_after: Option<&str>,
    ) -> Result<(Vec<GitLabIssue>, bool)> {
        let page_str = page.to_string();
        let project_id_str = project_id.to_string();
        let state = if updated_after.is_some() {
            "all"
        } else {
            "opened"
        };
        let mut params = vec![("state", state), ("per_page", "100"), ("page", &page_str)];
        if let Some(after) = updated_after {
            params.push(("updated_after", after));
        }

        let description = format!("GitLab issues page {page} for project {project_id}");
        let response = crate::services::http::fetch_with_retry(&description, 3, || {
            self.client
                .get(format!(
                    "{}/api/v4/projects/{project_id_str}/issues",
                    self.base_url
                ))
                .header("PRIVATE-TOKEN", &self.token)
                .query(&params)
                .send()
        })
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, page = page, "Failed to fetch issues page");
            Error::Api(e.to_string())
        })?;

        let body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, page = page, "Failed to read response body");
            e
        })?;

        let issues: Vec<GitLabIssue> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, body_preview = %&body[..body.len().min(200)], project_id = project_id, page = page, "Failed to decode issues");
            e
        })?;

        let has_more = !issues.is_empty();
        tracing::debug!(
            page = page,
            count = issues.len(),
            "Fetched GitLab issues page"
        );
        Ok((issues, has_more))
    }

    /// Fetch a single page of merge requests. Returns (items, has_more_pages).
    pub async fn fetch_merge_requests_page(
        &self,
        project_id: i64,
        page: u32,
        updated_after: Option<&str>,
    ) -> Result<(Vec<GitLabMergeRequest>, bool)> {
        let page_str = page.to_string();
        let project_id_str = project_id.to_string();
        let state = if updated_after.is_some() {
            "all"
        } else {
            "opened"
        };
        let mut params = vec![("state", state), ("per_page", "100"), ("page", &page_str)];
        if let Some(after) = updated_after {
            params.push(("updated_after", after));
        }

        let description = format!("GitLab merge requests page {page} for project {project_id}");
        let response = crate::services::http::fetch_with_retry(&description, 3, || {
            self.client
                .get(format!(
                    "{}/api/v4/projects/{project_id_str}/merge_requests",
                    self.base_url
                ))
                .header("PRIVATE-TOKEN", &self.token)
                .query(&params)
                .send()
        })
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, page = page, "Failed to fetch merge requests page");
            Error::Api(e.to_string())
        })?;

        let body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, page = page, "Failed to read response body");
            e
        })?;

        let mrs: Vec<GitLabMergeRequest> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, body_preview = %&body[..body.len().min(200)], project_id = project_id, page = page, "Failed to decode merge requests");
            e
        })?;

        let has_more = !mrs.is_empty();
        tracing::debug!(
            page = page,
            count = mrs.len(),
            "Fetched GitLab merge requests page"
        );
        Ok((mrs, has_more))
    }

    /// Fetch notes (comments) for a GitLab issue, excluding system notes.
    pub async fn get_issue_notes(
        &self,
        project_id: i64,
        issue_iid: i32,
    ) -> Result<Vec<GitLabNote>> {
        let project_id_str = project_id.to_string();
        let issue_iid_str = issue_iid.to_string();

        let description = format!("GitLab issue notes for project {project_id} issue {issue_iid}");
        let response = crate::services::http::fetch_with_retry(&description, 3, || {
            self.client
                .get(format!(
                    "{}/api/v4/projects/{project_id_str}/issues/{issue_iid_str}/notes",
                    self.base_url
                ))
                .header("PRIVATE-TOKEN", &self.token)
                .query(&[("per_page", "100"), ("sort", "asc")])
                .send()
        })
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, issue_iid = issue_iid, "Failed to fetch issue notes");
            Error::Api(e.to_string())
        })?;

        let body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, issue_iid = issue_iid, "Failed to read issue notes response body");
            e
        })?;

        let notes: Vec<GitLabNote> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, body_preview = %&body[..body.len().min(200)], project_id = project_id, issue_iid = issue_iid, "Failed to decode issue notes");
            e
        })?;

        let user_notes: Vec<GitLabNote> = notes.into_iter().filter(|n| !n.system).collect();
        tracing::debug!(
            project_id = project_id,
            issue_iid = issue_iid,
            count = user_notes.len(),
            "Fetched GitLab issue notes"
        );
        Ok(user_notes)
    }

    /// Fetch notes (comments) for a GitLab merge request, excluding system notes.
    pub async fn get_mr_notes(&self, project_id: i64, mr_iid: i32) -> Result<Vec<GitLabNote>> {
        let project_id_str = project_id.to_string();
        let mr_iid_str = mr_iid.to_string();

        let description = format!("GitLab MR notes for project {project_id} MR {mr_iid}");
        let response = crate::services::http::fetch_with_retry(&description, 3, || {
            self.client
                .get(format!(
                    "{}/api/v4/projects/{project_id_str}/merge_requests/{mr_iid_str}/notes",
                    self.base_url
                ))
                .header("PRIVATE-TOKEN", &self.token)
                .query(&[("per_page", "100"), ("sort", "asc")])
                .send()
        })
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, mr_iid = mr_iid, "Failed to fetch MR notes");
            Error::Api(e.to_string())
        })?;

        let body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, mr_iid = mr_iid, "Failed to read MR notes response body");
            e
        })?;

        let notes: Vec<GitLabNote> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, body_preview = %&body[..body.len().min(200)], project_id = project_id, mr_iid = mr_iid, "Failed to decode MR notes");
            e
        })?;

        let user_notes: Vec<GitLabNote> = notes.into_iter().filter(|n| !n.system).collect();
        tracing::debug!(
            project_id = project_id,
            mr_iid = mr_iid,
            count = user_notes.len(),
            "Fetched GitLab MR notes"
        );
        Ok(user_notes)
    }

    /// Fetch commits for a GitLab merge request.
    pub async fn get_mr_commits(&self, project_id: i64, mr_iid: i32) -> Result<Vec<GitLabCommit>> {
        let project_id_str = project_id.to_string();
        let mr_iid_str = mr_iid.to_string();

        let description = format!("GitLab MR commits for project {project_id} MR {mr_iid}");
        let response = crate::services::http::fetch_with_retry(&description, 3, || {
            self.client
                .get(format!(
                    "{}/api/v4/projects/{project_id_str}/merge_requests/{mr_iid_str}/commits",
                    self.base_url
                ))
                .header("PRIVATE-TOKEN", &self.token)
                .query(&[("per_page", "100")])
                .send()
        })
        .await
        .map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, mr_iid = mr_iid, "Failed to fetch MR commits");
            Error::Api(e.to_string())
        })?;

        let body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, project_id = project_id, mr_iid = mr_iid, "Failed to read MR commits response body");
            e
        })?;

        let commits: Vec<GitLabCommit> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, body_preview = %&body[..body.len().min(200)], project_id = project_id, mr_iid = mr_iid, "Failed to decode MR commits");
            e
        })?;

        tracing::debug!(
            project_id = project_id,
            mr_iid = mr_iid,
            count = commits.len(),
            "Fetched GitLab MR commits"
        );
        Ok(commits)
    }
}

#[async_trait]
impl IssueCreator for GitLabClient {
    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        request: &CreateIssueRequest,
    ) -> std::result::Result<CreateIssueResponse, String> {
        let path = format!("{owner}/{repo}");
        tracing::info!(path = %path, title = %request.title, "Creating GitLab issue");

        let project_id = self.get_project_id(&path).await.map_err(|e| {
            tracing::error!(error = %e, path = %path, "Failed to resolve GitLab project ID for issue creation");
            e.to_string()
        })?;

        let mut body = serde_json::json!({
            "title": request.title,
        });
        if let Some(ref description) = request.body {
            body["description"] = serde_json::Value::String(description.clone());
        }
        if let Some(ref labels) = request.labels {
            body["labels"] = serde_json::Value::String(labels.join(","));
        }

        let response = self
            .client
            .post(format!(
                "{}/api/v4/projects/{project_id}/issues",
                self.base_url
            ))
            .header("PRIVATE-TOKEN", &self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, path = %path, "GitLab API network error creating issue");
                e.to_string()
            })?;

        let status = response.status();
        let resp_body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, path = %path, "Failed to read create issue response body");
            e.to_string()
        })?;

        if !status.is_success() {
            tracing::error!(status = %status, path = %path, body = %resp_body, "GitLab API error creating issue");
            return Err(match status.as_u16() {
                403 => format!(
                    "Permission denied: your token doesn't have write access to {path}. \
                     Please check that your token has the 'api' or 'write_repository' scope."
                ),
                401 => "Authentication failed: your GitLab token is invalid or expired. Please update it in Settings.".to_string(),
                404 => format!("Project {path} not found, or your token doesn't have access to it."),
                422 => format!("GitLab rejected the issue: {resp_body}"),
                _ => format!("GitLab API error (HTTP {status}): {resp_body}"),
            });
        }

        #[derive(Deserialize)]
        struct GlIssueResponse {
            iid: i32,
            web_url: String,
        }

        let parsed: GlIssueResponse = serde_json::from_str(&resp_body).map_err(|e| {
            tracing::error!(error = %e, body_preview = %&resp_body[..resp_body.len().min(200)], path = %path, "Failed to decode create issue response");
            e.to_string()
        })?;

        tracing::info!(path = %path, iid = parsed.iid, "GitLab issue created");

        Ok(CreateIssueResponse {
            number: parsed.iid,
            url: parsed.web_url,
        })
    }
}
