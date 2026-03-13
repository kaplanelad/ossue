export interface ChatMessage {
  id: string;
  item_id: string;
  role: "user" | "assistant";
  content: string;
  created_at: string;
  input_tokens: number | null;
  output_tokens: number | null;
  model: string | null;
}

export type AnalysisAction = "analyze" | "draft_response";

export interface AnalyzeActionRequest {
  item_id: string;
  action: AnalysisAction;
  additional_context?: string;
}

export interface AiSettings {
  ai_mode: "api" | "cli";
  ai_provider: "api" | "claude_cli" | "cursor_cli";
  ai_model: string;
  has_ai_api_key: boolean;
  ai_focus_areas: string[];
  ai_review_strictness: "strict" | "pragmatic" | "lenient";
  ai_response_tone: "friendly" | "neutral" | "terse";
  ai_custom_instructions: string | null;
}
