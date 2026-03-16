import type { DraftIssueStatus } from './draft';

// --- ItemTypeData discriminated union (matches Rust serde output) ---

interface ProviderItemFields {
  external_id: number;
  state: "open" | "closed" | "merged";
  author: string;
  url: string;
  comments_count: number;
  fetched_at: string;
  labels?: string[];
}

export interface IssueTypeData extends ProviderItemFields {
  kind: "issue";
}

// PrItemData uses #[serde(flatten)] so provider fields are at top level
export interface PrTypeData extends ProviderItemFields {
  kind: "pr";
  pr_branch: string | null;
  pr_diff: string | null;
}

export interface DiscussionTypeData extends ProviderItemFields {
  kind: "discussion";
}

export interface NoteTypeData {
  kind: "note";
  raw_content: string;
  draft_status: DraftIssueStatus;
  labels: string[] | null;
  priority: string | null;
  area: string | null;
  provider_issue_number: number | null;
  provider_issue_url: string | null;
}

export type ItemTypeData = IssueTypeData | PrTypeData | DiscussionTypeData | NoteTypeData;

export type ItemType = "issue" | "pr" | "discussion" | "note";

export interface Item {
  id: string;
  project_id: string;
  item_type: ItemType;
  title: string;
  body: string;
  type_data: ItemTypeData;
  is_read: boolean;
  is_starred: boolean;
  is_deleted: boolean;
  item_status: "pending" | "resolved" | "dismissed" | "deleted";
  created_at: string;
  updated_at: string;
}

export interface DismissedCount {
  project_id: string;
  item_type: string;
  count: number;
}

export interface ItemTypeCount {
  project_id: string;
  item_type: string;
  count: number;
}

export interface ItemPageResponse {
  items: Item[];
  next_cursor: string | null;
  has_more: boolean;
  dismissed_counts: DismissedCount[];
  item_type_counts: ItemTypeCount[];
  starred_counts: ItemTypeCount[];
  analyzed_counts: ItemTypeCount[];
  draft_note_counts: ItemTypeCount[];
}
