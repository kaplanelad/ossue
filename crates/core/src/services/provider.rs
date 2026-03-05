use std::path::Path;

use crate::enums::ActionType;
use crate::services::ai_api::{AiApiService, ApiMessage};
use crate::services::context::{ContextService, ItemContext};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Api(#[from] crate::services::ai_api::Error),

    #[error("CLI execution failed: {0}")]
    Cli(String),

    #[error("CLI not found: {0}. Is it installed?")]
    CliNotFound(String),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum CliTool {
    ClaudeCode,
    Cursor,
}

impl CliTool {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude_cli" => Some(Self::ClaudeCode),
            "cursor_cli" => Some(Self::Cursor),
            _ => None,
        }
    }

    pub fn binary_name(&self) -> &str {
        match self {
            Self::ClaudeCode => "claude",
            Self::Cursor => "cursor",
        }
    }
}

/// Run analysis using the API provider (fast, limited context).
pub async fn analyze_with_api(
    service: &AiApiService,
    action: &ActionType,
    context: &ItemContext,
    diff: Option<&str>,
    history: &[ApiMessage],
) -> Result<reqwest::Response> {
    let system_prompt = ContextService::build_system_prompt(action, &context.item_type);
    let user_prompt = ContextService::build_action_prompt(action, context, diff);

    let mut messages = history.to_vec();
    messages.push(ApiMessage {
        role: "user".to_string(),
        content: user_prompt,
    });

    let request_service = AiApiService::new_with_system(
        service.api_key().to_string(),
        Some(service.model().to_string()),
        system_prompt,
    );

    let response = request_service.send_message_streaming(&messages).await?;

    Ok(response)
}

/// Run analysis using a CLI tool (deep, full repo context).
pub async fn analyze_with_cli(
    cli_tool: &CliTool,
    cli_path: Option<&str>,
    action: &ActionType,
    context: &ItemContext,
    diff: Option<&str>,
    repo_path: &Path,
    model: Option<&str>,
) -> Result<String> {
    let binary = cli_path.unwrap_or(cli_tool.binary_name());

    // Check if binary exists
    let which_result = tokio::process::Command::new("which")
        .arg(binary)
        .output()
        .await
        .map_err(|e| Error::Cli(e.to_string()))?;

    if !which_result.status.success() {
        return Err(Error::CliNotFound(binary.to_string()));
    }

    let prompt = ContextService::build_action_prompt(action, context, diff);

    let mut args = match cli_tool {
        CliTool::ClaudeCode => vec!["-p".to_string(), prompt],
        CliTool::Cursor => vec!["--message".to_string(), prompt],
    };
    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }

    tracing::info!(binary, action = %action, repo = %repo_path.display(), "Running CLI analysis");

    let output = tokio::process::Command::new(binary)
        .current_dir(repo_path)
        .args(&args)
        .output()
        .await
        .map_err(|e| Error::Cli(format!("Failed to run {binary}: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Cli(format!("{binary} error: {stderr}")));
    }

    let response = String::from_utf8_lossy(&output.stdout).to_string();
    tracing::info!(response_len = response.len(), "CLI analysis complete");
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("claude_cli", Some(CliTool::ClaudeCode))]
    #[case("cursor_cli", Some(CliTool::Cursor))]
    #[case("unknown", None)]
    fn cli_tool_from_str(#[case] input: &str, #[case] expected: Option<CliTool>) {
        assert_eq!(CliTool::from_str(input), expected);
    }

    #[rstest]
    #[case(CliTool::ClaudeCode, "claude")]
    #[case(CliTool::Cursor, "cursor")]
    fn cli_tool_binary_name(#[case] tool: CliTool, #[case] expected: &str) {
        assert_eq!(tool.binary_name(), expected);
    }
}
