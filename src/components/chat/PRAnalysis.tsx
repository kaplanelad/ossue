import type { ChatMessage } from "@/types";

interface PRAnalysisProps {
  message: ChatMessage;
}

export function PRAnalysis({ message: _message }: PRAnalysisProps) {
  // For now, PR analysis is rendered as markdown in AIMessage
  // This component can be enhanced later for structured table view
  return null;
}
