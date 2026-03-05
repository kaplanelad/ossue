export type DraftIssueStatus = "draft" | "ready" | "submitted";

export interface DraftIssue {
  id: string;
  project_id: string;
  status: DraftIssueStatus;
  raw_content: string;
  title: string | null;
  body: string | null;
  labels: string[] | null;
  priority: string | null;
  area: string | null;
  provider_issue_number: number | null;
  provider_issue_url: string | null;
  is_starred: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateIssueResponse {
  number: number;
  url: string;
}
