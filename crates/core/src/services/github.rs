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
pub struct GitHubClient {
    token: String,
    client: reqwest::Client,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRepo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub owner: GitHubOwner,
    pub stargazers_count: Option<i64>,
    pub default_branch: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubOwner {
    pub login: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubGraphQLIssue {
    pub number: i32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub url: String,
    pub author: Option<GitHubGraphQLAuthor>,
    pub comments: GitHubGraphQLCommentCount,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubGraphQLPullRequest {
    pub number: i32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub url: String,
    pub author: Option<GitHubGraphQLAuthor>,
    pub comments: GitHubGraphQLCommentCount,
    #[serde(rename = "headRefName")]
    pub head_ref_name: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubDiscussion {
    pub number: i32,
    pub title: String,
    pub body: Option<String>,
    pub closed: bool,
    pub url: String,
    pub author: Option<GitHubGraphQLAuthor>,
    pub comments: GitHubGraphQLCommentCount,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubGraphQLAuthor {
    pub login: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubGraphQLCommentCount {
    #[serde(rename = "totalCount")]
    pub total_count: i32,
}

#[derive(Debug, Deserialize)]
struct GraphQLDiscussionsResponse {
    data: Option<GraphQLDiscussionsData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLDiscussionsData {
    repository: GraphQLRepository,
}

#[derive(Debug, Deserialize)]
struct GraphQLRepository {
    discussions: GraphQLDiscussionConnection,
}

#[derive(Debug, Deserialize)]
struct GraphQLDiscussionConnection {
    nodes: Vec<GitHubDiscussion>,
    #[serde(rename = "pageInfo")]
    page_info: GraphQLPageInfo,
}

#[derive(Debug, Deserialize)]
struct GraphQLPageInfo {
    #[serde(rename = "hasNextPage")]
    has_next_page: bool,
    #[serde(rename = "endCursor")]
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLIssuesResponse {
    data: Option<GraphQLIssuesData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLIssuesData {
    repository: GraphQLIssuesRepository,
}

#[derive(Debug, Deserialize)]
struct GraphQLIssuesRepository {
    issues: GraphQLIssueConnection,
}

#[derive(Debug, Deserialize)]
struct GraphQLIssueConnection {
    nodes: Vec<GitHubGraphQLIssue>,
    #[serde(rename = "pageInfo")]
    page_info: GraphQLPageInfo,
}

#[derive(Debug, Deserialize)]
struct GraphQLPullRequestsResponse {
    data: Option<GraphQLPullRequestsData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLPullRequestsData {
    repository: GraphQLPullRequestsRepository,
}

#[derive(Debug, Deserialize)]
struct GraphQLPullRequestsRepository {
    #[serde(rename = "pullRequests")]
    pull_requests: GraphQLPullRequestConnection,
}

#[derive(Debug, Deserialize)]
struct GraphQLPullRequestConnection {
    nodes: Vec<GitHubGraphQLPullRequest>,
    #[serde(rename = "pageInfo")]
    page_info: GraphQLPageInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubComment {
    pub id: i64,
    pub body: Option<String>,
    pub user: GitHubOwner,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubReviewComment {
    pub id: i64,
    pub body: Option<String>,
    pub path: String,
    pub line: Option<i32>,
    pub user: GitHubOwner,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubCommit {
    pub sha: String,
    pub commit: GitHubCommitDetail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubCommitDetail {
    pub message: String,
    pub author: Option<GitHubCommitAuthor>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubCommitAuthor {
    pub name: String,
    pub date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubTimelineEvent {
    pub event: Option<String>,
    pub source: Option<GitHubTimelineSource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubTimelineSource {
    pub issue: Option<GitHubTimelineIssue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubTimelineIssue {
    pub number: i32,
    pub title: String,
    pub html_url: String,
    pub state: String,
}

impl GitHubClient {
    pub fn new(token: String) -> Self {
        Self::with_base_url(token, None)
    }

    pub fn with_base_url(token: String, base_url: Option<String>) -> Self {
        let base_url = match base_url {
            Some(url) => {
                let url = url.trim_end_matches('/').to_string();
                format!("{url}/api/v3")
            }
            None => "https://api.github.com".to_string(),
        };
        let client = reqwest::Client::builder()
            .user_agent("ossue")
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            token,
            client,
            base_url,
        }
    }

    pub async fn get_repo(&self, owner: &str, repo: &str) -> Result<GitHubRepo> {
        let url = format!("{}/repos/{owner}/{repo}", self.base_url);
        let response: GitHubRepo = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "ossue")
            .send()
            .await?
            .error_for_status()
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, "Failed to fetch repo metadata");
                e
            })?
            .json()
            .await?;
        Ok(response)
    }

    pub async fn list_repos(&self) -> Result<Vec<GitHubRepo>> {
        tracing::info!("Fetching GitHub repos");
        let mut all_repos = Vec::new();
        let mut page = 1;

        loop {
            let response = self
                .client
                .get(format!("{}/user/repos", self.base_url))
                .header("Authorization", format!("Bearer {}", self.token))
                .query(&[
                    ("per_page", "100"),
                    ("page", &page.to_string()),
                    ("sort", "updated"),
                    ("affiliation", "owner,collaborator,organization_member"),
                ])
                .send()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "GitHub API network error fetching repos");
                    e
                })?;

            let body = response
                .error_for_status()
                .inspect_err(|e| {
                    tracing::error!(status = ?e.status(), "GitHub API error fetching repos");
                })?
                .text()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to read GitHub repos response body");
                    e
                })?;

            let repos: Vec<GitHubRepo> = serde_json::from_str(&body)
                .map_err(|e| {
                    tracing::error!(error = %e, body_preview = %&body[..body.len().min(200)], "Failed to decode GitHub repos response");
                    e
                })?;

            tracing::debug!(
                page = page,
                count = repos.len(),
                "Fetched GitHub repos page"
            );

            if repos.is_empty() {
                break;
            }
            all_repos.extend(repos);
            page += 1;
            if page > MAX_PAGES {
                tracing::warn!("Reached maximum pagination limit");
                break;
            }
        }

        tracing::info!(total = all_repos.len(), "Fetched all GitHub repos");
        Ok(all_repos)
    }

    pub async fn list_discussions(&self, owner: &str, repo: &str) -> Result<Vec<GitHubDiscussion>> {
        tracing::info!("Fetching GitHub discussions for {}/{}", owner, repo);
        let mut all_discussions = Vec::new();
        let mut cursor: Option<String> = None;
        let mut iteration: u32 = 0;

        let query = r#"
            query($owner: String!, $repo: String!, $cursor: String) {
                repository(owner: $owner, name: $repo) {
                    discussions(first: 100, after: $cursor, orderBy: {field: UPDATED_AT, direction: DESC}) {
                        nodes {
                            number
                            title
                            body
                            closed
                            url
                            author { login }
                            comments { totalCount }
                            createdAt
                            updatedAt
                        }
                        pageInfo {
                            hasNextPage
                            endCursor
                        }
                    }
                }
            }
        "#;

        loop {
            let variables = serde_json::json!({
                "owner": owner,
                "repo": repo,
                "cursor": cursor,
            });

            let body = self
                .client
                .post(format!("{}/graphql", self.base_url))
                .header("Authorization", format!("Bearer {}", self.token))
                .json(&serde_json::json!({
                    "query": query,
                    "variables": variables,
                }))
                .send()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, owner = %owner, repo = %repo, "GitHub GraphQL API network error fetching discussions");
                    e
                })?
                .error_for_status()
                .inspect_err(|e| {
                    tracing::error!(status = ?e.status(), owner = %owner, repo = %repo, "GitHub GraphQL API error fetching discussions");
                })?
                .text()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, owner = %owner, repo = %repo, "Failed to read GitHub discussions response body");
                    e
                })?;

            let response: GraphQLDiscussionsResponse = serde_json::from_str(&body)
                .map_err(|e| {
                    tracing::error!(error = %e, owner = %owner, repo = %repo, body_preview = %&body[..body.len().min(200)], "Failed to decode GitHub discussions response");
                    e
                })?;

            if let Some(errors) = &response.errors {
                let msg = errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                tracing::error!(owner = %owner, repo = %repo, errors = %msg, "GitHub GraphQL API returned errors");
                return Err(Error::Decode(serde_json::from_str::<()>(&msg).unwrap_err()));
            }

            let data = response.data.ok_or_else(|| {
                tracing::error!(owner = %owner, repo = %repo, "GitHub GraphQL response missing data field");
                Error::Decode(serde_json::from_str::<()>("missing data").unwrap_err())
            })?;

            let connection = data.repository.discussions;
            tracing::debug!(
                count = connection.nodes.len(),
                "Fetched GitHub discussions page"
            );

            if connection.nodes.is_empty() {
                break;
            }

            all_discussions.extend(connection.nodes);

            iteration += 1;
            if connection.page_info.has_next_page {
                cursor = connection.page_info.end_cursor;
            } else {
                break;
            }
            if iteration >= MAX_PAGES {
                tracing::warn!("Reached maximum pagination limit");
                break;
            }
        }

        // Filter out closed discussions client-side (GraphQL API doesn't support server-side filtering)
        all_discussions.retain(|d| !d.closed);
        tracing::info!(
            total = all_discussions.len(),
            "Fetched all GitHub discussions (open only)"
        );
        Ok(all_discussions)
    }

    /// Fetch a single page of issues using GraphQL cursor-based pagination.
    /// Returns (items, has_next_page, end_cursor).
    pub async fn fetch_issues_page(
        &self,
        owner: &str,
        repo: &str,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<GitHubGraphQLIssue>, bool, Option<String>)> {
        let query = r#"
            query($owner: String!, $repo: String!, $cursor: String, $states: [IssueState!], $filterBy: IssueFilters) {
                repository(owner: $owner, name: $repo) {
                    issues(first: 100, after: $cursor, orderBy: {field: UPDATED_AT, direction: DESC}, states: $states, filterBy: $filterBy) {
                        nodes {
                            number
                            title
                            body
                            state
                            url
                            author { login }
                            comments { totalCount }
                            createdAt
                            updatedAt
                        }
                        pageInfo {
                            hasNextPage
                            endCursor
                        }
                    }
                }
            }
        "#;

        let states = if since.is_some() {
            serde_json::Value::Null
        } else {
            serde_json::json!(["OPEN"])
        };
        let filter_by = since.map(|s| serde_json::json!({"since": s}));

        let variables = serde_json::json!({
            "owner": owner,
            "repo": repo,
            "cursor": cursor,
            "states": states,
            "filterBy": filter_by,
        });

        let description = format!(
            "GitHub issues page for {owner}/{repo} (cursor: {})",
            cursor.unwrap_or("none")
        );
        let response = crate::services::http::fetch_with_retry(&description, 3, || {
            self.client
                .post(format!("{}/graphql", self.base_url))
                .header("Authorization", format!("Bearer {}", self.token))
                .json(&serde_json::json!({
                    "query": query,
                    "variables": variables,
                }))
                .send()
        })
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

        let body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, owner = %owner, repo = %repo, "Failed to read issues response body");
            e
        })?;

        let graphql_response: GraphQLIssuesResponse =
            serde_json::from_str(&body).map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, body_preview = %&body[..body.len().min(200)], "Failed to decode issues response");
                e
            })?;

        if let Some(errors) = &graphql_response.errors {
            let msg = errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            tracing::error!(owner = %owner, repo = %repo, errors = %msg, "GitHub GraphQL API returned errors");
            return Err(Error::Api(msg));
        }

        let data = graphql_response.data.ok_or_else(|| {
            tracing::error!(owner = %owner, repo = %repo, "GitHub GraphQL response missing data field");
            Error::Api("GitHub GraphQL response missing data field".to_string())
        })?;

        let connection = data.repository.issues;
        let has_next_page = connection.page_info.has_next_page;
        let end_cursor = connection.page_info.end_cursor;

        tracing::debug!(
            count = connection.nodes.len(),
            has_next_page = has_next_page,
            "Fetched GitHub issues page"
        );

        Ok((connection.nodes, has_next_page, end_cursor))
    }

    /// Fetch a single page of pull requests using GraphQL cursor-based pagination.
    /// Returns (items, has_next_page, end_cursor).
    pub async fn fetch_pull_requests_page(
        &self,
        owner: &str,
        repo: &str,
        cursor: Option<&str>,
        since: Option<&str>,
    ) -> Result<(Vec<GitHubGraphQLPullRequest>, bool, Option<String>)> {
        let query = r#"
            query($owner: String!, $repo: String!, $cursor: String, $states: [PullRequestState!]) {
                repository(owner: $owner, name: $repo) {
                    pullRequests(first: 100, after: $cursor, orderBy: {field: UPDATED_AT, direction: DESC}, states: $states) {
                        nodes {
                            number
                            title
                            body
                            state
                            url
                            author { login }
                            comments { totalCount }
                            headRefName
                            createdAt
                            updatedAt
                        }
                        pageInfo {
                            hasNextPage
                            endCursor
                        }
                    }
                }
            }
        "#;

        let states = if since.is_some() {
            serde_json::json!(["OPEN", "CLOSED", "MERGED"])
        } else {
            serde_json::json!(["OPEN"])
        };

        let variables = serde_json::json!({
            "owner": owner,
            "repo": repo,
            "cursor": cursor,
            "states": states,
        });

        let description = format!(
            "GitHub PRs page for {owner}/{repo} (cursor: {})",
            cursor.unwrap_or("none")
        );
        let response = crate::services::http::fetch_with_retry(&description, 3, || {
            self.client
                .post(format!("{}/graphql", self.base_url))
                .header("Authorization", format!("Bearer {}", self.token))
                .json(&serde_json::json!({
                    "query": query,
                    "variables": variables,
                }))
                .send()
        })
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

        let body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, owner = %owner, repo = %repo, "Failed to read PRs response body");
            e
        })?;

        let graphql_response: GraphQLPullRequestsResponse =
            serde_json::from_str(&body).map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, body_preview = %&body[..body.len().min(200)], "Failed to decode PRs response");
                e
            })?;

        if let Some(errors) = &graphql_response.errors {
            let msg = errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            tracing::error!(owner = %owner, repo = %repo, errors = %msg, "GitHub GraphQL API returned errors");
            return Err(Error::Api(msg));
        }

        let data = graphql_response.data.ok_or_else(|| {
            tracing::error!(owner = %owner, repo = %repo, "GitHub GraphQL response missing data field");
            Error::Api("GitHub GraphQL response missing data field".to_string())
        })?;

        let connection = data.repository.pull_requests;
        let has_next_page = connection.page_info.has_next_page;
        let end_cursor = connection.page_info.end_cursor;

        tracing::debug!(
            count = connection.nodes.len(),
            has_next_page = has_next_page,
            "Fetched GitHub PRs page"
        );

        Ok((connection.nodes, has_next_page, end_cursor))
    }

    /// Fetch a single page of discussions using GraphQL cursor-based pagination.
    /// Returns (items, has_next_page, end_cursor).
    pub async fn fetch_discussions_page(
        &self,
        owner: &str,
        repo: &str,
        cursor: Option<&str>,
    ) -> Result<(Vec<GitHubDiscussion>, bool, Option<String>)> {
        let query = r#"
            query($owner: String!, $repo: String!, $cursor: String) {
                repository(owner: $owner, name: $repo) {
                    discussions(first: 100, after: $cursor, orderBy: {field: UPDATED_AT, direction: DESC}) {
                        nodes {
                            number
                            title
                            body
                            closed
                            url
                            author { login }
                            comments { totalCount }
                            createdAt
                            updatedAt
                        }
                        pageInfo {
                            hasNextPage
                            endCursor
                        }
                    }
                }
            }
        "#;

        let variables = serde_json::json!({
            "owner": owner,
            "repo": repo,
            "cursor": cursor,
        });

        let description = format!(
            "GitHub discussions page for {owner}/{repo} (cursor: {})",
            cursor.unwrap_or("none")
        );
        let response = crate::services::http::fetch_with_retry(&description, 3, || {
            self.client
                .post(format!("{}/graphql", self.base_url))
                .header("Authorization", format!("Bearer {}", self.token))
                .json(&serde_json::json!({
                    "query": query,
                    "variables": variables,
                }))
                .send()
        })
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

        let body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, owner = %owner, repo = %repo, "Failed to read discussions response body");
            e
        })?;

        let graphql_response: GraphQLDiscussionsResponse =
            serde_json::from_str(&body).map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, body_preview = %&body[..body.len().min(200)], "Failed to decode discussions response");
                e
            })?;

        if let Some(errors) = &graphql_response.errors {
            let msg = errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            tracing::error!(owner = %owner, repo = %repo, errors = %msg, "GitHub GraphQL API returned errors");
            return Err(Error::Api(msg));
        }

        let data = graphql_response.data.ok_or_else(|| {
            tracing::error!(owner = %owner, repo = %repo, "GitHub GraphQL response missing data field");
            Error::Api("GitHub GraphQL response missing data field".to_string())
        })?;

        let connection = data.repository.discussions;
        let has_next_page = connection.page_info.has_next_page;
        let end_cursor = connection.page_info.end_cursor;

        tracing::debug!(
            count = connection.nodes.len(),
            has_next_page = has_next_page,
            "Fetched GitHub discussions page"
        );

        Ok((connection.nodes, has_next_page, end_cursor))
    }

    pub async fn get_pr_diff(&self, owner: &str, repo: &str, pr_number: i32) -> Result<String> {
        tracing::debug!(owner = %owner, repo = %repo, pr_number = pr_number, "Fetching PR diff");
        let diff = self
            .client
            .get(format!(
                "{}/repos/{owner}/{repo}/pulls/{pr_number}", self.base_url
            ))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3.diff")
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, pr_number = pr_number, "GitHub API network error fetching PR diff");
                e
            })?
            .error_for_status()
            .inspect_err(|e| {
                tracing::error!(status = ?e.status(), owner = %owner, repo = %repo, pr_number = pr_number, "GitHub API error fetching PR diff");
            })?
            .text()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, pr_number = pr_number, "Failed to read PR diff response body");
                e
            })?;

        Ok(diff)
    }

    pub async fn get_issue_comments(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
    ) -> Result<Vec<GitHubComment>> {
        tracing::debug!(owner = %owner, repo = %repo, issue_number = issue_number, "Fetching issue comments");
        let response = self
            .client
            .get(format!(
                "{}/repos/{owner}/{repo}/issues/{issue_number}/comments", self.base_url
            ))
            .header("Authorization", format!("Bearer {}", self.token))
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, issue_number = issue_number, "GitHub API network error fetching issue comments");
                e
            })?;

        let body = response
            .error_for_status()
            .inspect_err(|e| {
                tracing::error!(status = ?e.status(), owner = %owner, repo = %repo, issue_number = issue_number, "GitHub API error fetching issue comments");
            })?
            .text()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, issue_number = issue_number, "Failed to read issue comments response body");
                e
            })?;

        let comments: Vec<GitHubComment> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, owner = %owner, repo = %repo, issue_number = issue_number, body_preview = %&body[..body.len().min(200)], "Failed to decode issue comments response");
            e
        })?;

        tracing::debug!(count = comments.len(), owner = %owner, repo = %repo, issue_number = issue_number, "Fetched issue comments");
        Ok(comments)
    }

    pub async fn get_pr_review_comments(
        &self,
        owner: &str,
        repo: &str,
        pr_number: i32,
    ) -> Result<Vec<GitHubReviewComment>> {
        tracing::debug!(owner = %owner, repo = %repo, pr_number = pr_number, "Fetching PR review comments");
        let response = self
            .client
            .get(format!(
                "{}/repos/{owner}/{repo}/pulls/{pr_number}/comments", self.base_url
            ))
            .header("Authorization", format!("Bearer {}", self.token))
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, pr_number = pr_number, "GitHub API network error fetching PR review comments");
                e
            })?;

        let body = response
            .error_for_status()
            .inspect_err(|e| {
                tracing::error!(status = ?e.status(), owner = %owner, repo = %repo, pr_number = pr_number, "GitHub API error fetching PR review comments");
            })?
            .text()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, pr_number = pr_number, "Failed to read PR review comments response body");
                e
            })?;

        let comments: Vec<GitHubReviewComment> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, owner = %owner, repo = %repo, pr_number = pr_number, body_preview = %&body[..body.len().min(200)], "Failed to decode PR review comments response");
            e
        })?;

        tracing::debug!(count = comments.len(), owner = %owner, repo = %repo, pr_number = pr_number, "Fetched PR review comments");
        Ok(comments)
    }

    pub async fn get_pr_commits(
        &self,
        owner: &str,
        repo: &str,
        pr_number: i32,
    ) -> Result<Vec<GitHubCommit>> {
        tracing::debug!(owner = %owner, repo = %repo, pr_number = pr_number, "Fetching PR commits");
        let response = self
            .client
            .get(format!(
                "{}/repos/{owner}/{repo}/pulls/{pr_number}/commits", self.base_url
            ))
            .header("Authorization", format!("Bearer {}", self.token))
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, pr_number = pr_number, "GitHub API network error fetching PR commits");
                e
            })?;

        let body = response
            .error_for_status()
            .inspect_err(|e| {
                tracing::error!(status = ?e.status(), owner = %owner, repo = %repo, pr_number = pr_number, "GitHub API error fetching PR commits");
            })?
            .text()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, pr_number = pr_number, "Failed to read PR commits response body");
                e
            })?;

        let commits: Vec<GitHubCommit> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, owner = %owner, repo = %repo, pr_number = pr_number, body_preview = %&body[..body.len().min(200)], "Failed to decode PR commits response");
            e
        })?;

        tracing::debug!(count = commits.len(), owner = %owner, repo = %repo, pr_number = pr_number, "Fetched PR commits");
        Ok(commits)
    }

    pub async fn get_issue_timeline(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
    ) -> Result<Vec<GitHubTimelineEvent>> {
        tracing::debug!(owner = %owner, repo = %repo, issue_number = issue_number, "Fetching issue timeline");
        let response = self
            .client
            .get(format!(
                "{}/repos/{owner}/{repo}/issues/{issue_number}/timeline", self.base_url
            ))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.mockingbird-preview+json")
            .query(&[("per_page", "100")])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, issue_number = issue_number, "GitHub API network error fetching issue timeline");
                e
            })?;

        let body = response
            .error_for_status()
            .inspect_err(|e| {
                tracing::error!(status = ?e.status(), owner = %owner, repo = %repo, issue_number = issue_number, "GitHub API error fetching issue timeline");
            })?
            .text()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, issue_number = issue_number, "Failed to read issue timeline response body");
                e
            })?;

        let events: Vec<GitHubTimelineEvent> = serde_json::from_str(&body).map_err(|e| {
            tracing::error!(error = %e, owner = %owner, repo = %repo, issue_number = issue_number, body_preview = %&body[..body.len().min(200)], "Failed to decode issue timeline response");
            e
        })?;

        tracing::debug!(count = events.len(), owner = %owner, repo = %repo, issue_number = issue_number, "Fetched issue timeline");
        Ok(events)
    }
}

#[async_trait]
impl IssueCreator for GitHubClient {
    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        request: &CreateIssueRequest,
    ) -> std::result::Result<CreateIssueResponse, String> {
        tracing::info!(owner = %owner, repo = %repo, title = %request.title, "Creating GitHub issue");

        let mut body = serde_json::json!({
            "title": request.title,
        });
        if let Some(ref b) = request.body {
            body["body"] = serde_json::Value::String(b.clone());
        }
        if let Some(ref labels) = request.labels {
            body["labels"] = serde_json::json!(labels);
        }

        let response = self
            .client
            .post(format!(
                "{}/repos/{owner}/{repo}/issues", self.base_url
            ))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "ossue")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, owner = %owner, repo = %repo, "GitHub API network error creating issue");
                e.to_string()
            })?;

        let status = response.status();
        let resp_body = response.text().await.map_err(|e| {
            tracing::error!(error = %e, owner = %owner, repo = %repo, "Failed to read create issue response body");
            e.to_string()
        })?;

        if !status.is_success() {
            tracing::error!(status = %status, owner = %owner, repo = %repo, body = %resp_body, "GitHub API error creating issue");

            // Try to extract the error message from GitHub's JSON response
            let api_message = serde_json::from_str::<serde_json::Value>(&resp_body)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from));

            return Err(match status.as_u16() {
                403 => {
                    let detail = api_message.as_deref().unwrap_or("");
                    if detail.contains("Resource not accessible by personal access token") {
                        format!(
                            "Permission denied: classic personal access tokens may be restricted by this organization. \
                             Try using a fine-grained token with 'Issues: Read and write' permission, \
                             or check the organization's token policy in Settings > Third-party access."
                        )
                    } else {
                        format!(
                            "Permission denied: your token doesn't have write access to {owner}/{repo}. \
                             Please check that your token has the 'repo' scope (or 'Issues: Read and write' for fine-grained tokens). \
                             GitHub: {detail}"
                        )
                    }
                },
                401 => "Authentication failed: your GitHub token is invalid or expired. Please update it in Settings.".to_string(),
                404 => format!("Repository {owner}/{repo} not found, or your token doesn't have access to it."),
                410 => format!("Issues are disabled for {owner}/{repo}. Enable them in the repository settings."),
                422 => {
                    let detail = api_message.unwrap_or_else(|| resp_body.clone());
                    format!("GitHub rejected the issue: {detail}")
                },
                _ => format!("GitHub API error (HTTP {status}): {resp_body}"),
            });
        }

        #[derive(Deserialize)]
        struct GhIssueResponse {
            number: i32,
            html_url: String,
        }

        let parsed: GhIssueResponse = serde_json::from_str(&resp_body).map_err(|e| {
            tracing::error!(error = %e, body_preview = %&resp_body[..resp_body.len().min(200)], owner = %owner, repo = %repo, "Failed to decode create issue response");
            e.to_string()
        })?;

        tracing::info!(owner = %owner, repo = %repo, number = parsed.number, "GitHub issue created");

        Ok(CreateIssueResponse {
            number: parsed.number,
            url: parsed.html_url,
        })
    }
}
