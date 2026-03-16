export type AiMode = "api" | "cli";
export type AiProvider = "api" | "claude_cli" | "cursor_cli";
export type ReviewStrictness = "strict" | "pragmatic" | "lenient";
export type ResponseTone = "friendly" | "neutral" | "terse";

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
  ai_mode: AiMode;
  ai_provider: AiProvider;
  ai_model: string;
  has_ai_api_key: boolean;
  ai_focus_areas: string[];
  ai_review_strictness: ReviewStrictness;
  ai_response_tone: ResponseTone;
  ai_custom_instructions: string | null;
}
