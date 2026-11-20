import { ApiError } from '../api';
export async function communityApi<T>(path: string, method = 'GET', body?: unknown): Promise<T> {
  const response = await fetch(`/api/community${path}`, {
    method,
    credentials: 'same-origin',
    headers: body === undefined ? {} : { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: 'Unable to connect' }));
    throw new ApiError(error.error || 'Unable to complete this request', response.status);
  }
  return response.json();
}
export interface Member {
  id: string;
  display_name: string;
  avatar_url: string;
}
export interface ServerChoice {
  id: string | null;
  external_id: string;
  name: string;
  connected: boolean;
  can_manage: boolean;
}
export interface Server {
  id: string;
  name: string;
  channel_id: string | null;
  channel_name: string;
  enabled: boolean;
  mentions_enabled: boolean;
  ai_tags_enabled?: boolean;
}
export interface Artist {
  category: string;
  enabled: boolean;
  avatar_url: string;
  official_links: { label: string; url: string }[];
  id: string;
  name: string;
  url: string;
  source: string;
  favorite: boolean;
  notify: boolean;
  last_error: string | null;
}
export interface Post {
  category?: string;
  tags?: string[];
  tags_source?: string | null;
  id: string;
  artist_id: string;
  title: string;
  author: string;
  url: string;
  summary: string;
  media: { url: string; kind: string }[];
  feed_name: string;
  published_at: string;
  liked: boolean;
  favorite: boolean;
  likes: number;
}
export interface Page {
  items: Post[];
  total: number;
  page: number;
  pages: number;
}
export interface Delivery {
  id: string;
  title: string;
  artist: string;
  status: string;
  attempts: number;
  last_error: string | null;
  created_at: string;
}
