use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::enums::{ActionType, ItemType};
use crate::services::github::GitHubClient;
use crate::services::gitlab::GitLabClient;
use crate::services::repo_manager::{ProjectFiles, RepoManager};

// ---------------------------------------------------------------------------
// Data structures for assembled context
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemContext {
    pub title: String,
    pub body: String,
    pub item_type: ItemType,
    pub author: String,
    pub url: String,
    pub state: String,
    pub comments: Vec<ContextComment>,
    pub commits: Vec<ContextCommit>,
    pub linked_issues: Vec<LinkedIssue>,
    pub project_files: Option<ContextProjectFiles>,
    pub maintainer_notes: Vec<String>,
    pub custom_instructions: Option<String>,
    pub focus_areas: Vec<String>,
    pub review_strictness: Option<String>,
    pub response_tone: Option<String>,
    pub pr_diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextComment {
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub path: Option<String>,
    pub line: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCommit {
    pub sha: String,
    pub message: String,
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedIssue {
    pub number: i32,
    pub title: String,
    pub url: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextProjectFiles {
    pub contributing: Option<String>,
    pub pr_template: Option<String>,
    pub readme_excerpt: Option<String>,
}

// ---------------------------------------------------------------------------
// Conversion from RepoManager's ProjectFiles
// ---------------------------------------------------------------------------

impl From<ProjectFiles> for ContextProjectFiles {
    fn from(pf: ProjectFiles) -> Self {
        Self {
            contributing: pf.contributing,
            pr_template: pf.pr_template,
            readme_excerpt: pf.readme_excerpt,
        }
    }
}

// ---------------------------------------------------------------------------
// ContextService
// ---------------------------------------------------------------------------

pub struct ContextService;

impl ContextService {
    // -----------------------------------------------------------------------
    // GitHub context gathering
    // -----------------------------------------------------------------------

    /// Gather rich context for a GitHub item (issue or pull request).
    ///
    /// Fetches comments, review comments (for PRs), commits (for PRs), and
    /// timeline (cross-references) from the GitHub API. Reads project files
    /// from the local repo clone when `repo_path` is provided.
    ///
    /// Individual API calls are allowed to fail without aborting the whole
    /// context assembly -- errors are logged and the corresponding section is
    /// left empty.
    pub async fn gather_github_context(
        client: &GitHubClient,
        owner: &str,
        repo: &str,
        item_type: &ItemType,
        external_id: i32,
        repo_path: Option<&Path>,
        default_branch: Option<&str>,
    ) -> ItemContext {
        let mut comments: Vec<ContextComment> = Vec::new();
        let mut commits: Vec<ContextCommit> = Vec::new();
        let mut linked_issues: Vec<LinkedIssue> = Vec::new();

        // --- Issue comments (available for both issues and PRs) ---
        if let Ok(issue_comments) = client.get_issue_comments(owner, repo, external_id).await {
            for c in issue_comments {
                comments.push(ContextComment {
                    author: c.user.login.clone(),
                    body: c.body.unwrap_or_default(),
                    created_at: c.created_at.clone(),
                    path: None,
                    line: None,
                });
            }
        } else {
            tracing::warn!(
                owner = %owner,
                repo = %repo,
                external_id = external_id,
                "Failed to fetch GitHub issue comments for context"
            );
        }

        // --- PR-specific enrichment ---
        let mut pr_diff: Option<String> = None;

        if *item_type == ItemType::PullRequest {
            // Review comments (inline code comments)
            if let Ok(review_comments) = client
                .get_pr_review_comments(owner, repo, external_id)
                .await
            {
                for rc in review_comments {
                    comments.push(ContextComment {
                        author: rc.user.login.clone(),
                        body: rc.body.unwrap_or_default(),
                        created_at: rc.created_at.clone(),
                        path: Some(rc.path.clone()),
                        line: rc.line,
                    });
                }
            } else {
                tracing::warn!(
                    owner = %owner,
                    repo = %repo,
                    external_id = external_id,
                    "Failed to fetch GitHub PR review comments for context"
                );
            }

            // Commits
            if let Ok(pr_commits) = client.get_pr_commits(owner, repo, external_id).await {
                for c in pr_commits {
                    commits.push(ContextCommit {
                        sha: c.sha.clone(),
                        message: c.commit.message.clone(),
                        author: c
                            .commit
                            .author
                            .as_ref()
                            .map(|a| a.name.clone())
                            .unwrap_or_default(),
                    });
                }
            } else {
                tracing::warn!(
                    owner = %owner,
                    repo = %repo,
                    external_id = external_id,
                    "Failed to fetch GitHub PR commits for context"
                );
            }

            // PR diff
            tracing::info!(owner = %owner, repo = %repo, pr_number = external_id, "Fetching PR diff");
            match client.get_pr_diff(owner, repo, external_id).await {
                Ok(diff) => {
                    // Truncate very large diffs to stay within token budget
                    const MAX_DIFF_CHARS: usize = 200_000;
                    if diff.len() > MAX_DIFF_CHARS {
                        tracing::warn!(
                            owner = %owner,
                            repo = %repo,
                            pr_number = external_id,
                            original_len = diff.len(),
                            "Truncating large PR diff to {} chars", MAX_DIFF_CHARS
                        );
                        pr_diff = Some(diff.chars().take(MAX_DIFF_CHARS).collect());
                    } else {
                        pr_diff = Some(diff);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        owner = %owner,
                        repo = %repo,
                        external_id = external_id,
                        error = %e,
                        "Failed to fetch PR diff, continuing without it"
                    );
                }
            }
        }

        // --- Timeline (cross-references / linked issues) ---
        if let Ok(timeline_events) = client.get_issue_timeline(owner, repo, external_id).await {
            for event in timeline_events {
                if event.event.as_deref() == Some("cross-referenced") {
                    if let Some(source) = &event.source {
                        if let Some(issue) = &source.issue {
                            linked_issues.push(LinkedIssue {
                                number: issue.number,
                                title: issue.title.clone(),
                                url: issue.html_url.clone(),
                                state: issue.state.clone(),
                            });
                        }
                    }
                }
            }
        } else {
            tracing::warn!(
                owner = %owner,
                repo = %repo,
                external_id = external_id,
                "Failed to fetch GitHub issue timeline for context"
            );
        }

        // --- Project files from repo clone ---
        // Use default branch ref for project files so PR checkouts don't pollute them
        let project_files = repo_path.map(|path| {
            let pf = match default_branch {
                Some(branch) => RepoManager::read_project_context_from_ref(path, branch),
                None => RepoManager::read_project_context(path),
            };
            ContextProjectFiles::from(pf)
        });

        ItemContext {
            title: String::new(),
            body: String::new(),
            item_type: item_type.clone(),
            author: String::new(),
            url: String::new(),
            state: String::new(),
            comments,
            commits,
            linked_issues,
            project_files,
            maintainer_notes: Vec::new(),
            custom_instructions: None,
            focus_areas: Vec::new(),
            review_strictness: None,
            response_tone: None,
            pr_diff,
        }
    }

    // -----------------------------------------------------------------------
    // GitLab context gathering
    // -----------------------------------------------------------------------

    /// Gather rich context for a GitLab item (issue or merge request).
    ///
    /// Fetches notes and commits (for MRs) from the GitLab API. Reads project
    /// files from the local repo clone when `repo_path` is provided.
    pub async fn gather_gitlab_context(
        client: &GitLabClient,
        project_id: i64,
        item_type: &ItemType,
        external_id: i32,
        repo_path: Option<&Path>,
        default_branch: Option<&str>,
    ) -> ItemContext {
        let mut comments: Vec<ContextComment> = Vec::new();
        let mut commits: Vec<ContextCommit> = Vec::new();

        match item_type {
            ItemType::Issue => {
                if let Ok(notes) = client.get_issue_notes(project_id, external_id).await {
                    for n in notes {
                        comments.push(ContextComment {
                            author: n.author.username.clone(),
                            body: n.body.clone(),
                            created_at: n.created_at.clone(),
                            path: None,
                            line: None,
                        });
                    }
                } else {
                    tracing::warn!(
                        project_id = project_id,
                        external_id = external_id,
                        "Failed to fetch GitLab issue notes for context"
                    );
                }
            }
            ItemType::PullRequest => {
                // MR notes
                if let Ok(notes) = client.get_mr_notes(project_id, external_id).await {
                    for n in notes {
                        comments.push(ContextComment {
                            author: n.author.username.clone(),
                            body: n.body.clone(),
                            created_at: n.created_at.clone(),
                            path: None,
                            line: None,
                        });
                    }
                } else {
                    tracing::warn!(
                        project_id = project_id,
                        external_id = external_id,
                        "Failed to fetch GitLab MR notes for context"
                    );
                }

                // MR commits
                if let Ok(mr_commits) = client.get_mr_commits(project_id, external_id).await {
                    for c in mr_commits {
                        commits.push(ContextCommit {
                            sha: c.id.clone(),
                            message: c.message.clone(),
                            author: c.author_name.clone(),
                        });
                    }
                } else {
                    tracing::warn!(
                        project_id = project_id,
                        external_id = external_id,
                        "Failed to fetch GitLab MR commits for context"
                    );
                }
            }
            ItemType::Discussion | ItemType::Note => {
                // GitLab discussions and notes don't have a separate notes endpoint.
                tracing::debug!(
                    project_id = project_id,
                    external_id = external_id,
                    "Skipping notes fetch for GitLab discussion/note"
                );
            }
        }

        // --- Project files from repo clone ---
        // Use default branch ref for project files so MR checkouts don't pollute them
        let project_files = repo_path.map(|path| {
            let pf = match default_branch {
                Some(branch) => RepoManager::read_project_context_from_ref(path, branch),
                None => RepoManager::read_project_context(path),
            };
            ContextProjectFiles::from(pf)
        });

        ItemContext {
            title: String::new(),
            body: String::new(),
            item_type: item_type.clone(),
            author: String::new(),
            url: String::new(),
            state: String::new(),
            comments,
            commits,
            linked_issues: Vec::new(),
            project_files,
            maintainer_notes: Vec::new(),
            custom_instructions: None,
            focus_areas: Vec::new(),
            review_strictness: None,
            response_tone: None,
            pr_diff: None,
        }
    }

    // -----------------------------------------------------------------------
    // Prompt building
    // -----------------------------------------------------------------------

    /// Build a system prompt tailored to the requested action.
    pub fn build_system_prompt(action: &ActionType, item_type: &ItemType) -> String {
        match action {
            ActionType::Analyze => {
                match item_type {
                    ItemType::PullRequest => {
                        "You are an expert code reviewer and maintainer's assistant. \
                         Analyze this pull request: summarize the changes, assess impact \
                         and code quality, and suggest a review comment."
                            .to_string()
                    }
                    ItemType::Discussion => {
                        "You are a maintainer's assistant. Analyze this discussion and give \
                         the maintainer everything they need to take action: what it's about, \
                         whether maintainer input is needed, and a suggested response."
                            .to_string()
                    }
                    _ => {
                        "You are a maintainer's assistant. Analyze this issue and give \
                         the maintainer everything they need to take action: what it's about, \
                         how urgent it is, and a suggested response."
                            .to_string()
                    }
                }
            }
            ActionType::DraftResponse => {
                match item_type {
                    ItemType::Issue => {
                        "You are an experienced open source maintainer drafting a \
                         response to an issue reporter. Be welcoming, clear, and helpful. \
                         If more info is needed, ask specific questions."
                            .to_string()
                    }
                    ItemType::Discussion => {
                        "You are an experienced open source maintainer responding to a \
                         community discussion. Address the user's actual problem. Point to \
                         relevant docs or settings. Be welcoming to community participation."
                            .to_string()
                    }
                    _ => {
                        "You are an experienced open source maintainer drafting a \
                         response to a contributor. Be welcoming, clear, and helpful. \
                         Reference project guidelines when relevant."
                            .to_string()
                    }
                }
            }
        }
    }

    /// Build a structured action prompt that includes all available context.
    ///
    /// The prompt is assembled in sections so the AI can reference each part
    /// independently. Sections that have no data are omitted to keep the
    /// prompt concise.
    pub fn build_action_prompt(
        action: &ActionType,
        context: &ItemContext,
        diff: Option<&str>,
    ) -> String {
        let mut sections: Vec<String> = Vec::new();

        // 1. Action-specific instructions
        sections.push(format!(
            "## Action\n{}",
            Self::action_instructions(action, &context.item_type)
        ));

        // 2. Custom instructions
        if let Some(ref instructions) = context.custom_instructions {
            if !instructions.is_empty() {
                sections.push(format!("## Custom Instructions\n{}", instructions));
            }
        }

        // 3. Review strictness / response tone
        if let Some(ref strictness) = context.review_strictness {
            sections.push(format!("## Review Strictness\n{}", strictness));
        }
        if let Some(ref tone) = context.response_tone {
            sections.push(format!("## Response Tone\n{}", tone));
        }

        // 4. Focus areas
        if !context.focus_areas.is_empty() {
            sections.push(format!(
                "## Focus Areas\n- {}",
                context.focus_areas.join("\n- ")
            ));
        }

        // 5. Project notes & context (maintainer notes + draft notes)
        if !context.maintainer_notes.is_empty() {
            sections.push(format!(
                "## Project Notes & Context\n{}",
                context
                    .maintainer_notes
                    .iter()
                    .map(|n| format!("- {n}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        // 6. CONTRIBUTING.md excerpt
        if let Some(ref pf) = context.project_files {
            if let Some(ref contributing) = pf.contributing {
                // Truncate long files to keep within token budget
                let excerpt: String = contributing.chars().take(3000).collect();
                sections.push(format!("## CONTRIBUTING.md\n{}", excerpt));
            }
        }

        // 7. Item details
        sections.push(format!(
            "## Item Details\n\
             - **Type:** {}\n\
             - **Title:** {}\n\
             - **Author:** {}\n\
             - **State:** {}\n\
             - **URL:** {}",
            context.item_type, context.title, context.author, context.state, context.url,
        ));

        // 8. Item body
        if !context.body.is_empty() {
            sections.push(format!("## Description\n{}", context.body));
        }

        // 9. Diff
        if let Some(d) = diff {
            if !d.is_empty() {
                sections.push(format!("## Diff\n```diff\n{}\n```", d));
            }
        }

        // 10. Commits
        if !context.commits.is_empty() {
            let commit_lines: Vec<String> = context
                .commits
                .iter()
                .map(|c| {
                    format!(
                        "- `{}` {} ({})",
                        &c.sha[..c.sha.len().min(8)],
                        c.message.lines().next().unwrap_or(""),
                        c.author
                    )
                })
                .collect();
            sections.push(format!("## Commits\n{}", commit_lines.join("\n")));
        }

        // 11. Discussion thread
        if !context.comments.is_empty() {
            let comment_lines: Vec<String> = context
                .comments
                .iter()
                .map(|c| {
                    let location = match (&c.path, c.line) {
                        (Some(path), Some(line)) => format!(" ({}:{})", path, line),
                        (Some(path), None) => format!(" ({})", path),
                        _ => String::new(),
                    };
                    format!(
                        "### @{} ({}){}\n{}",
                        c.author, c.created_at, location, c.body
                    )
                })
                .collect();
            sections.push(format!(
                "## Discussion Thread\n{}",
                comment_lines.join("\n\n")
            ));
        }

        // 12. Linked issues
        if !context.linked_issues.is_empty() {
            let issue_lines: Vec<String> = context
                .linked_issues
                .iter()
                .map(|li| format!("- #{} {} [{}]({})", li.number, li.title, li.state, li.url))
                .collect();
            sections.push(format!("## Linked Issues\n{}", issue_lines.join("\n")));
        }

        sections.join("\n\n")
    }

    /// Return action-specific instructions embedded in the prompt.
    fn action_instructions(action: &ActionType, item_type: &ItemType) -> String {
        match action {
            ActionType::Analyze => {
                match item_type {
                    ItemType::PullRequest => {
                        "Analyze this pull request for a busy maintainer. Structure your response as:\n\n\
                         ## At a Glance\n\
                         **Verdict:** CAN MERGE / NEEDS CHANGES / NEEDS DISCUSSION\n\
                         **Breaking changes:** Yes — [brief] / None found\n\
                         **Risk:** Low / Medium / High\n\n\
                         ## What's Going On\n\
                         Summarize what this PR does and the state of discussion in 1-3 sentences.\n\n\
                         ## Key Findings\n\
                         | Severity | File | Line | Finding |\n\
                         |----------|------|------|---------|\n\
                         Critical bugs, security issues, performance concerns. Reference specific lines from the diff.\n\n\
                         ## Action Items\n\
                         - [ ] Required changes before merge (if any)\n\n\
                         ## Suggested Review Comment\n\
                         A ready-to-paste review comment."
                            .to_string()
                    }
                    ItemType::Discussion => {
                        "Analyze this discussion for a busy maintainer. Structure your response as:\n\n\
                         ## At a Glance\n\
                         **Topic:** Configuration | Bug help | Feature idea | General\n\
                         **Needs maintainer input:** Yes / No\n\
                         **Community sentiment:** Positive / Neutral / Frustrated\n\n\
                         ## What's Going On\n\
                         Summarize the discussion and any back-and-forth in 1-3 sentences. If the thread is long, distill the key points and latest status.\n\n\
                         ## Key Findings\n\
                         - User's actual problem (distilled from back-and-forth)\n\
                         - Relevant docs/settings/code pointers\n\
                         - Should this be converted to an issue?\n\n\
                         ## Suggested Response\n\
                         A ready-to-paste response that is welcoming, clear, and addresses the discussion directly."
                            .to_string()
                    }
                    _ => {
                        "Analyze this issue for a busy maintainer. Structure your response as:\n\n\
                         ## At a Glance\n\
                         **Type:** Bug report | Feature request | Question | Support\n\
                         **Priority:** Critical / High / Medium / Low\n\
                         **Action:** Response needed / Can close / Needs more info\n\n\
                         ## What's Going On\n\
                         Summarize the issue and any discussion in 1-3 sentences. If the thread is long, distill the key points and latest status.\n\n\
                         ## Key Findings\n\
                         - Is this a duplicate of a known issue?\n\
                         - Does existing documentation cover this?\n\
                         - What info is missing from the reporter?\n\
                         - Recommended labels (bug, enhancement, good first issue, etc.)\n\n\
                         ## Suggested Response\n\
                         A ready-to-paste response that is welcoming, clear, and addresses the issue directly. If more info is needed, ask specific questions."
                            .to_string()
                    }
                }
            }
            ActionType::DraftResponse => {
                match item_type {
                    ItemType::Issue => {
                        "Draft a response to this issue from the perspective of a project maintainer. \
                         Be welcoming and helpful. If more info is needed, ask specific questions. \
                         If this is a known issue or has a workaround, mention it. \
                         If this can be closed, explain why."
                            .to_string()
                    }
                    ItemType::Discussion => {
                        "Draft a response to this discussion from the perspective of a project maintainer. \
                         Address the user's actual problem. Point to relevant docs or settings. \
                         If this is a feature request, indicate feasibility and timeline expectations. \
                         Be welcoming to community participation."
                            .to_string()
                    }
                    _ => {
                        "Draft a response to this item from the perspective of a project maintainer. \
                         Be polite, welcoming, and constructive. If this is a PR, mention what \
                         needs to change before it can be merged (if anything)."
                            .to_string()
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // -----------------------------------------------------------------------
    // build_system_prompt
    // -----------------------------------------------------------------------

    #[rstest]
    #[case(ItemType::Issue, "issue")]
    #[case(ItemType::Discussion, "discussion")]
    #[case(ItemType::PullRequest, "code reviewer")]
    fn build_system_prompt_analyze_by_item_type(
        #[case] item_type: ItemType,
        #[case] expected_fragment: &str,
    ) {
        let prompt = ContextService::build_system_prompt(&ActionType::Analyze, &item_type);
        assert!(
            prompt.to_lowercase().contains(expected_fragment),
            "prompt for Analyze/{item_type:?} should contain '{expected_fragment}'"
        );
    }

    #[rstest]
    #[case(ItemType::Issue, "issue")]
    #[case(ItemType::Discussion, "discussion")]
    #[case(ItemType::PullRequest, "contributor")]
    fn build_system_prompt_draft_response_by_item_type(
        #[case] item_type: ItemType,
        #[case] expected_fragment: &str,
    ) {
        let prompt = ContextService::build_system_prompt(&ActionType::DraftResponse, &item_type);
        assert!(
            prompt.to_lowercase().contains(expected_fragment),
            "prompt for DraftResponse/{item_type:?} should contain '{expected_fragment}'"
        );
    }

    // -----------------------------------------------------------------------
    // build_action_prompt
    // -----------------------------------------------------------------------

    fn full_context() -> ItemContext {
        ItemContext {
            title: "Test Title".to_string(),
            body: "Test body content".to_string(),
            item_type: ItemType::PullRequest,
            author: "testuser".to_string(),
            url: "https://github.com/test/repo/pull/1".to_string(),
            state: "open".to_string(),
            comments: vec![ContextComment {
                author: "reviewer".to_string(),
                body: "Looks good".to_string(),
                created_at: "2024-01-15".to_string(),
                path: Some("src/main.rs".to_string()),
                line: Some(42),
            }],
            commits: vec![ContextCommit {
                sha: "abc12345".to_string(),
                message: "fix: resolve bug".to_string(),
                author: "dev".to_string(),
            }],
            linked_issues: vec![LinkedIssue {
                number: 10,
                title: "Original bug".to_string(),
                url: "https://github.com/test/repo/issues/10".to_string(),
                state: "open".to_string(),
            }],
            project_files: Some(ContextProjectFiles {
                contributing: Some("Please follow the guidelines".to_string()),
                pr_template: None,
                readme_excerpt: None,
            }),
            maintainer_notes: vec!["Note 1".to_string()],
            custom_instructions: Some("Be strict".to_string()),
            focus_areas: vec!["security".to_string(), "performance".to_string()],
            review_strictness: Some("high".to_string()),
            response_tone: Some("professional".to_string()),
            pr_diff: None,
        }
    }

    fn minimal_context() -> ItemContext {
        ItemContext {
            title: "Minimal".to_string(),
            body: String::new(),
            item_type: ItemType::Issue,
            author: "user".to_string(),
            url: "https://example.com".to_string(),
            state: "open".to_string(),
            comments: vec![],
            commits: vec![],
            linked_issues: vec![],
            project_files: None,
            maintainer_notes: vec![],
            custom_instructions: None,
            focus_areas: vec![],
            review_strictness: None,
            response_tone: None,
            pr_diff: None,
        }
    }

    #[test]
    fn build_action_prompt_includes_all_sections() {
        let ctx = full_context();
        let prompt =
            ContextService::build_action_prompt(&ActionType::Analyze, &ctx, Some("diff here"));

        assert!(prompt.contains("## Action"));
        assert!(prompt.contains("## Custom Instructions"));
        assert!(prompt.contains("Be strict"));
        assert!(prompt.contains("## Review Strictness"));
        assert!(prompt.contains("## Response Tone"));
        assert!(prompt.contains("## Focus Areas"));
        assert!(prompt.contains("security"));
        assert!(prompt.contains("performance"));
        assert!(prompt.contains("## Project Notes & Context"));
        assert!(prompt.contains("## CONTRIBUTING.md"));
        assert!(prompt.contains("## Item Details"));
        assert!(prompt.contains("Test Title"));
        assert!(prompt.contains("## Description"));
        assert!(prompt.contains("## Diff"));
        assert!(prompt.contains("diff here"));
        assert!(prompt.contains("## Commits"));
        assert!(prompt.contains("abc12345"));
        assert!(prompt.contains("## Discussion Thread"));
        assert!(prompt.contains("@reviewer"));
        assert!(prompt.contains("## Linked Issues"));
        assert!(prompt.contains("#10"));
    }

    #[test]
    fn build_action_prompt_omits_empty_sections() {
        let ctx = minimal_context();
        let prompt = ContextService::build_action_prompt(&ActionType::Analyze, &ctx, None);

        assert!(prompt.contains("## Action"));
        assert!(prompt.contains("## Item Details"));
        // These should NOT appear
        assert!(!prompt.contains("## Custom Instructions"));
        assert!(!prompt.contains("## Review Strictness"));
        assert!(!prompt.contains("## Response Tone"));
        assert!(!prompt.contains("## Focus Areas"));
        assert!(!prompt.contains("## Project Notes & Context"));
        assert!(!prompt.contains("## CONTRIBUTING.md"));
        assert!(!prompt.contains("## Description"));
        assert!(!prompt.contains("## Diff"));
        assert!(!prompt.contains("## Commits"));
        assert!(!prompt.contains("## Discussion Thread"));
        assert!(!prompt.contains("## Linked Issues"));
    }

    #[test]
    fn build_action_prompt_truncates_contributing() {
        let long_contributing = "x".repeat(5000);
        let mut ctx = minimal_context();
        ctx.project_files = Some(ContextProjectFiles {
            contributing: Some(long_contributing),
            pr_template: None,
            readme_excerpt: None,
        });

        let prompt = ContextService::build_action_prompt(&ActionType::Analyze, &ctx, None);

        // CONTRIBUTING.md section should exist but be truncated to 3000 chars
        assert!(prompt.contains("## CONTRIBUTING.md"));
        let contributing_section = prompt
            .split("## CONTRIBUTING.md\n")
            .nth(1)
            .unwrap()
            .split("\n\n##")
            .next()
            .unwrap();
        assert!(contributing_section.len() <= 3000);
    }

    #[test]
    fn build_action_prompt_includes_custom_instructions() {
        let mut ctx = minimal_context();
        ctx.custom_instructions = Some("Always check for SQL injection".to_string());

        let prompt = ContextService::build_action_prompt(&ActionType::Analyze, &ctx, None);
        assert!(prompt.contains("## Custom Instructions"));
        assert!(prompt.contains("Always check for SQL injection"));
    }

    #[test]
    fn build_action_prompt_includes_focus_areas() {
        let mut ctx = minimal_context();
        ctx.focus_areas = vec!["security".to_string(), "performance".to_string()];

        let prompt = ContextService::build_action_prompt(&ActionType::Analyze, &ctx, None);
        assert!(prompt.contains("## Focus Areas"));
        assert!(prompt.contains("- security"));
        assert!(prompt.contains("- performance"));
    }

    // -----------------------------------------------------------------------
    // ContextProjectFiles From<ProjectFiles>
    // -----------------------------------------------------------------------

    #[test]
    fn context_project_files_from() {
        let pf = ProjectFiles {
            contributing: Some("contrib".to_string()),
            pr_template: Some("template".to_string()),
            readme_excerpt: None,
        };
        let cpf = ContextProjectFiles::from(pf);
        assert_eq!(cpf.contributing.as_deref(), Some("contrib"));
        assert_eq!(cpf.pr_template.as_deref(), Some("template"));
        assert!(cpf.readme_excerpt.is_none());
    }
}
