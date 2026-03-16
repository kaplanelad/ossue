export type Platform = "github" | "gitlab";

export interface Connector {
  id: string;
  name: string;
  platform: Platform;
  has_token: boolean;
  token_preview: string;
  base_url: string | null;
  created_at: string;
  updated_at: string;
}

export interface ConnectorRepo {
  name: string;
  full_name: string;
  url: string;
  description: string | null;
  owner: string;
  stars: number | null;
}

export interface GitHubRepo {
  id: number;
  name: string;
  full_name: string;
  html_url: string;
  description: string | null;
  owner: { login: string };
}

export interface GitLabProject {
  id: number;
  name: string;
  path_with_namespace: string;
  web_url: string;
  description: string | null;
  namespace: { path: string };
}
