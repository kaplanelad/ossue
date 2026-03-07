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
// Multi-step analysis
// ---------------------------------------------------------------------------

/// A single step in a multi-step analysis flow.
pub struct AnalysisStep {
    /// Shown in chat UI and saved to DB as the user message content.
    pub display_label: String,
    /// Full prompt sent to the LLM (includes hidden context on step 1).
    pub user_prompt: String,
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
        let mut pr_diff: Option<String> = None;

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

                // MR diff
                tracing::debug!(
                    project_id = project_id,
                    mr_iid = external_id,
                    "Fetching MR diff"
                );
                match client.get_mr_diff(project_id, external_id).await {
                    Ok(diff) => {
                        const MAX_DIFF_CHARS: usize = 200_000;
                        if diff.len() > MAX_DIFF_CHARS {
                            tracing::warn!(
                                project_id = project_id,
                                mr_iid = external_id,
                                original_len = diff.len(),
                                "Truncating large MR diff to {} chars",
                                MAX_DIFF_CHARS
                            );
                            pr_diff = Some(diff.chars().take(MAX_DIFF_CHARS).collect());
                        } else {
                            pr_diff = Some(diff);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            project_id = project_id,
                            external_id = external_id,
                            error = %e,
                            "Failed to fetch MR diff, continuing without it"
                        );
                    }
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
            pr_diff,
        }
    }

    // -----------------------------------------------------------------------
    // Multi-step analysis
    // -----------------------------------------------------------------------

    /// Build a system prompt for the multi-step analysis flow.
    /// This is a single system prompt used for the entire conversation.
    pub fn build_multi_step_system_prompt(item_type: &ItemType) -> String {
        let type_label = match item_type {
            ItemType::Issue => "issue",
            ItemType::Discussion => "discussion",
            ItemType::PullRequest => "pull request",
            ItemType::Note => "item",
        };
        format!(
            "You are a maintainer's assistant analyzing a {type_label}.\n\n\
             STRICT RULES:\n\
             1. Be direct and concise. No filler, no sugar-coating.\n\
             2. NEVER reproduce diff code, code blocks, or raw context data unless specifically asked. Reference files and lines by name only.\n\
             3. Follow the formatting instructions given in each user message exactly."
        )
    }

    /// Build the analysis steps for a multi-step analyze flow.
    ///
    /// Step 1 always includes the full context (via `build_action_prompt`).
    /// Subsequent steps are lighter instructions that rely on the LLM's
    /// conversation memory from step 1.
    pub fn build_analysis_steps(
        item_type: &ItemType,
        context: &ItemContext,
        diff: Option<&str>,
    ) -> Vec<AnalysisStep> {
        // Step 1 context: reuse existing build_action_prompt for the rich context,
        // but prepend step-specific formatting instructions.
        let rich_context = Self::build_action_prompt(&ActionType::Analyze, context, diff);

        match item_type {
            ItemType::PullRequest => vec![
                AnalysisStep {
                    display_label: "Summarize this pull request".to_string(),
                    user_prompt: format!(
                        "Summarize this pull request.\n\n\
                         Format your response as:\n\
                         ## At a Glance\n\
                         **Verdict:** CAN MERGE / NEEDS CHANGES / NEEDS DISCUSSION\n\
                         **Breaking changes:** Yes — [brief] / None found\n\
                         **Risk:** Low / Medium / High\n\n\
                         ## What's Going On\n\
                         1-3 sentences max. What does this PR do? If there are review comments, what's still open?\n\n\
                         ---\n\n{rich_context}"
                    ),
                },
                AnalysisStep {
                    display_label: "Review the code changes".to_string(),
                    user_prompt:
                        "Now review the code changes. Present findings as:\n\n\
                         ## Key Findings\n\
                         | Severity | File | Line | Finding |\n\
                         |----------|------|------|---------|\n\n\
                         Only real issues. If nothing, write \"No issues found.\" Do NOT pad with minor style nits.\n\n\
                         ## Action Items\n\
                         - [ ] What must happen before merge. If nothing, write \"Ready to merge.\""
                            .to_string(),
                },
                AnalysisStep {
                    display_label: "Draft a suggested review comment".to_string(),
                    user_prompt:
                        "Draft a ready-to-paste GitHub review comment based on your analysis. \
                         Direct, concise, actionable. No compliments or pleasantries — just what needs to happen. \
                         If the PR is ready to merge, say so briefly."
                            .to_string(),
                },
            ],
            ItemType::Discussion => vec![
                AnalysisStep {
                    display_label: "Summarize this discussion".to_string(),
                    user_prompt: format!(
                        "Summarize this discussion.\n\n\
                         Format your response as:\n\
                         ## At a Glance\n\
                         **Topic:** Configuration | Bug help | Feature idea | General\n\
                         **Needs maintainer input:** Yes / No\n\
                         **Community sentiment:** Positive / Neutral / Frustrated\n\n\
                         ## What's Going On\n\
                         1-3 sentences max. What is this really about and where does it stand?\n\n\
                         ---\n\n{rich_context}"
                    ),
                },
                AnalysisStep {
                    display_label: "Draft a suggested response".to_string(),
                    user_prompt:
                        "Draft a ready-to-paste response to this discussion. \
                         Address the user's actual problem. Point to relevant docs or settings. \
                         Be welcoming to community participation. Direct and helpful."
                            .to_string(),
                },
            ],
            // Issue, Note, or anything else: 2 steps
            _ => vec![
                AnalysisStep {
                    display_label: "Summarize this issue".to_string(),
                    user_prompt: format!(
                        "Summarize this issue.\n\n\
                         Format your response as:\n\
                         ## At a Glance\n\
                         **Type:** Bug report | Feature request | Question | Support\n\
                         **Priority:** Critical / High / Medium / Low\n\
                         **Action:** Response needed / Can close / Needs more info\n\n\
                         ## What's Going On\n\
                         1-3 sentences max. What is this really about? If there are comments, what was tried or left unresolved?\n\n\
                         ## Key Findings\n\
                         - Is this a duplicate?\n\
                         - Does existing docs cover this?\n\
                         - What info is missing?\n\
                         - Recommended labels\n\n\
                         ---\n\n{rich_context}"
                    ),
                },
                AnalysisStep {
                    display_label: "Draft a suggested response".to_string(),
                    user_prompt:
                        "Draft a ready-to-paste response to this issue from the perspective of a project maintainer. \
                         Be welcoming, clear, and helpful. If more info is needed, ask specific questions. \
                         If this is a known issue or has a workaround, mention it."
                            .to_string(),
                },
            ],
        }
    }

    // -----------------------------------------------------------------------
    // Prompt building
    // -----------------------------------------------------------------------

    /// Build a system prompt tailored to the requested action.
    pub fn build_system_prompt(action: &ActionType, item_type: &ItemType) -> String {
        match action {
            ActionType::Analyze => match item_type {
                ItemType::PullRequest => {
                    "You are a maintainer's assistant.\n\n\
                     STRICT RULES — VIOLATING ANY OF THESE IS A FAILURE:\n\
                     1. Your FIRST line of output MUST be \"## At a Glance\". No text before it. No introduction, no thinking, no meta-commentary, no \"Let me analyze\", no observations about the worktree or context. NOTHING before \"## At a Glance\".\n\
                     2. Output ONLY the sections defined below. No extra sections like \"Positive Observations\", \"Summary\", \"Overview\", \"Notes\", etc.\n\
                     3. NEVER reproduce diff code, code blocks, or raw context data. Reference files and lines by name only.\n\
                     4. Be direct and concise. No filler, no sugar-coating, no \"Great work on...\". State facts.\n\n\
                     ## At a Glance\n\
                     **Verdict:** CAN MERGE / NEEDS CHANGES / NEEDS DISCUSSION\n\
                     **Breaking changes:** Yes — [brief] / None found\n\
                     **Risk:** Low / Medium / High\n\n\
                     ## What's Going On\n\
                     1-3 sentences max. What does this PR do? If there are review comments, what's still open?\n\n\
                     ## Key Findings\n\
                     | Severity | File | Line | Finding |\n\
                     |----------|------|------|---------|\n\
                     Only real issues. If nothing, write \"No issues found.\" Do NOT pad with minor style nits.\n\n\
                     ## Action Items\n\
                     - [ ] What must happen before merge. If nothing, write \"Ready to merge.\"\n\n\
                     ## Suggested Review Comment\n\
                     A ready-to-paste GitHub review comment. Direct, concise, actionable. No compliments or pleasantries — just what needs to happen."
                        .to_string()
                }
                ItemType::Discussion => {
                    "You are a maintainer's assistant.\n\n\
                     STRICT RULES — VIOLATING ANY OF THESE IS A FAILURE:\n\
                     1. Your FIRST line of output MUST be \"## At a Glance\". No text before it. No introduction, no thinking, no meta-commentary. NOTHING before \"## At a Glance\".\n\
                     2. Output ONLY the sections defined below. No extra sections.\n\
                     3. NEVER reproduce raw context data.\n\
                     4. Be direct and concise. No filler.\n\n\
                     ## At a Glance\n\
                     **Topic:** Configuration | Bug help | Feature idea | General\n\
                     **Needs maintainer input:** Yes / No\n\
                     **Community sentiment:** Positive / Neutral / Frustrated\n\n\
                     ## What's Going On\n\
                     1-3 sentences max. What is this really about and where does it stand?\n\n\
                     ## Key Findings\n\
                     - The user's actual problem\n\
                     - Relevant docs, settings, or code pointers\n\
                     - Should this be converted to an issue?\n\n\
                     ## Suggested Response\n\
                     A ready-to-paste comment. Direct, helpful, moves things forward."
                        .to_string()
                }
                _ => {
                    "You are a maintainer's assistant.\n\n\
                     STRICT RULES — VIOLATING ANY OF THESE IS A FAILURE:\n\
                     1. Your FIRST line of output MUST be \"## At a Glance\". No text before it. No introduction, no thinking, no meta-commentary. NOTHING before \"## At a Glance\".\n\
                     2. Output ONLY the sections defined below. No extra sections.\n\
                     3. NEVER reproduce raw context data.\n\
                     4. Be direct and concise. No filler.\n\n\
                     ## At a Glance\n\
                     **Type:** Bug report | Feature request | Question | Support\n\
                     **Priority:** Critical / High / Medium / Low\n\
                     **Action:** Response needed / Can close / Needs more info\n\n\
                     ## What's Going On\n\
                     1-3 sentences max. What is this really about? If there are comments, what was tried or left unresolved?\n\n\
                     ## Key Findings\n\
                     - Is this a duplicate?\n\
                     - Does existing docs cover this?\n\
                     - What info is missing?\n\
                     - Recommended labels\n\n\
                     ## Suggested Response\n\
                     A ready-to-paste comment. Direct, helpful, moves things forward."
                        .to_string()
                }
            },
            ActionType::DraftResponse => match item_type {
                ItemType::Issue => "You are an experienced open source maintainer drafting a \
                         response to an issue reporter. Be welcoming, clear, and helpful. \
                         If more info is needed, ask specific questions."
                    .to_string(),
                ItemType::Discussion => {
                    "You are an experienced open source maintainer responding to a \
                         community discussion. Address the user's actual problem. Point to \
                         relevant docs or settings. Be welcoming to community participation."
                        .to_string()
                }
                _ => "You are an experienced open source maintainer drafting a \
                         response to a contributor. Be welcoming, clear, and helpful. \
                         Reference project guidelines when relevant."
                    .to_string(),
            },
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

        // 1. Action-specific instructions (only for non-Analyze — Analyze format is in the system prompt)
        if *action != ActionType::Analyze {
            sections.push(format!(
                "## Action\n{}",
                Self::action_instructions(action, &context.item_type)
            ));
        }

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

        // 6. CONTRIBUTING.md excerpt (only for DraftResponse — not useful for Analyze)
        if *action == ActionType::DraftResponse {
            if let Some(ref pf) = context.project_files {
                if let Some(ref contributing) = pf.contributing {
                    let excerpt: String = contributing.chars().take(3000).collect();
                    sections.push(format!("## CONTRIBUTING.md\n{}", excerpt));
                }
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
                "Analyze the item below using the output format from your instructions. Fill in every section with real values — do not echo the template."
                    .to_string()
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
    #[case(ItemType::Issue, "response needed")]
    #[case(ItemType::Discussion, "community sentiment")]
    #[case(ItemType::PullRequest, "can merge")]
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

        // Analyze: includes everything except CONTRIBUTING.md and Action section
        let prompt =
            ContextService::build_action_prompt(&ActionType::Analyze, &ctx, Some("diff here"));
        assert!(!prompt.contains("## Action"));
        assert!(prompt.contains("## Custom Instructions"));
        assert!(prompt.contains("Be strict"));
        assert!(prompt.contains("## Review Strictness"));
        assert!(prompt.contains("## Response Tone"));
        assert!(prompt.contains("## Focus Areas"));
        assert!(prompt.contains("security"));
        assert!(prompt.contains("performance"));
        assert!(prompt.contains("## Project Notes & Context"));
        assert!(!prompt.contains("## CONTRIBUTING.md"));
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

        // DraftResponse: includes CONTRIBUTING.md
        let prompt_dr = ContextService::build_action_prompt(
            &ActionType::DraftResponse,
            &ctx,
            Some("diff here"),
        );
        assert!(prompt_dr.contains("## CONTRIBUTING.md"));
    }

    #[test]
    fn build_action_prompt_omits_empty_sections() {
        let ctx = minimal_context();
        let prompt = ContextService::build_action_prompt(&ActionType::Analyze, &ctx, None);

        assert!(!prompt.contains("## Action"));
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

        let prompt = ContextService::build_action_prompt(&ActionType::DraftResponse, &ctx, None);

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
