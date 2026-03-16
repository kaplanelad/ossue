use serde::{Deserialize, Serialize};

use crate::enums::{ItemType, MessageRole};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error("Failed to decode response")]
    Decode(#[from] serde_json::Error),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct AiApiService {
    api_key: String,
    model: String,
    client: reqwest::Client,
    system_prompt_override: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Usage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse {
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct ContentBlock {
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: StreamMessage },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: DeltaBlock },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: serde_json::Value,
        #[serde(default)]
        usage: Usage,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: ApiError },
}

#[derive(Debug, Deserialize)]
pub struct StreamMessage {
    pub id: String,
    #[serde(default)]
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct DeltaBlock {
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub message: String,
}

impl AiApiService {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            api_key,
            model: model.unwrap_or_else(|| "claude-sonnet-4-6".to_string()),
            client,
            system_prompt_override: None,
        }
    }

    pub fn new_with_system(api_key: String, model: Option<String>, system_prompt: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            api_key,
            model: model.unwrap_or_else(|| "claude-sonnet-4-6".to_string()),
            client,
            system_prompt_override: Some(system_prompt),
        }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn build_system_prompt(&self) -> String {
        if let Some(ref override_prompt) = self.system_prompt_override {
            return override_prompt.clone();
        }
        "You are an AI assistant for open source maintainers. You help analyze issues, pull requests, and discussions.\n\
         \n\
         When analyzing a Pull Request, structure your response EXACTLY as follows:\n\
         \n\
         ## Verdict\n\
         [CAN MERGE / NEEDS CHANGES / NEEDS DISCUSSION] — one-line reason\n\
         \n\
         ## Breaking Changes\n\
         List any breaking changes with affected APIs, or 'None found' if none.\n\
         \n\
         ## Critical Findings\n\
         | Severity | File | Line | Finding |\n\
         |----------|------|------|---------|\n\
         (table of issues found, or 'No critical issues found')\n\
         \n\
         ## Action Items\n\
         - [ ] Checklist of things that must happen before merge\n\
         \n\
         ## Summary\n\
         2-3 sentence overview of what this PR does.\n\
         \n\
         ## Suggested Review Comment\n\
         A ready-to-paste GitHub review comment.\n\
         \n\
         For Issues, structure your response as:\n\
         \n\
         ## Quick Triage\n\
         **Type:** Bug report | Feature request | Question | Support\n\
         **Priority:** Critical / High / Medium / Low\n\
         **Action needed:** Yes — response required / No — can be closed / Needs more info\n\
         \n\
         ## One-liner\n\
         What this issue is about in 1 sentence.\n\
         \n\
         ## Context for Response\n\
         Relevant context: duplicates, docs coverage, missing info from reporter.\n\
         \n\
         ## Suggested Response\n\
         A ready-to-paste response.\n\
         \n\
         For Discussions, structure your response as:\n\
         \n\
         ## Quick Summary\n\
         **Topic:** Configuration | Bug help | Feature idea | General\n\
         **Needs maintainer input:** Yes / No\n\
         **Community sentiment:** Positive / Neutral / Frustrated\n\
         \n\
         ## One-liner\n\
         What this discussion is about.\n\
         \n\
         ## Maintainer Context\n\
         User's actual problem, relevant docs/settings/code pointers.\n\
         \n\
         ## Suggested Response\n\
         A ready-to-paste response.\n\
         \n\
         Be concise, direct, and actionable. Format responses in Markdown."
            .to_string()
    }

    pub fn build_analysis_prompt(
        item_type: &ItemType,
        title: &str,
        body: &str,
        diff: Option<&str>,
    ) -> String {
        let type_label = match item_type {
            ItemType::Issue => "issue",
            ItemType::PullRequest => "pull request",
            ItemType::Discussion => "discussion",
            ItemType::Note => "note",
        };
        let mut prompt = format!(
            "Please analyze this {type_label}:\n\n**Title:** {title}\n\n**Description:**\n{body}"
        );

        if let Some(diff) = diff {
            prompt.push_str(&format!("\n\n**Diff:**\n```diff\n{diff}\n```"));
        }

        prompt
    }

    pub async fn send_message(&self, messages: &[ApiMessage]) -> Result<(String, Usage)> {
        tracing::debug!(model = %self.model, max_tokens = 4096, "Sending AI message");
        let request = ApiRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: self.build_system_prompt(),
            messages: messages.to_vec(),
            stream: false,
        };

        let body = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, model = %self.model, "AI API network error");
                e
            })?
            .error_for_status()
            .inspect_err(|e| {
                tracing::error!(status = ?e.status(), model = %self.model, "AI API request failed");
            })?
            .text()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, model = %self.model, "Failed to read AI API response body");
                e
            })?;

        let response: ApiResponse = serde_json::from_str(&body)
            .map_err(|e| {
                tracing::error!(error = %e, body_preview = %&body[..body.len().min(500)], "Failed to decode AI API response");
                e
            })?;

        let usage = response.usage.clone();
        let result = response
            .content
            .into_iter()
            .map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");
        tracing::debug!(response_len = result.len(), "AI response received");
        Ok((result, usage))
    }

    pub async fn send_message_streaming(
        &self,
        messages: &[ApiMessage],
    ) -> Result<reqwest::Response> {
        tracing::info!(model = %self.model, "Starting streaming AI request");
        let request = ApiRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: self.build_system_prompt(),
            messages: messages.to_vec(),
            stream: true,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Streaming AI API network error");
                e
            })?
            .error_for_status()
            .inspect_err(|e| {
                tracing::error!(status = ?e.status(), "Streaming AI API request failed");
            })?;

        Ok(response)
    }
}
