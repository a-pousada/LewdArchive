export interface Feed {
  id: string;
  name: string;
  url: string;
  source: string;
  enabled: boolean;
  auto_download: boolean;
  notify: boolean;
  poll_minutes: number;
  initialized: boolean;
  last_checked_at: string | null;
  next_check_at: string;
  last_error: string | null;
  created_at: string;
}
export interface Entry {
  id: string;
  feed_id: string;
  title: string;
  author: string;
  url: string;
  summary: string;
  media: { url: string; kind: string }[];
  published_at: string;
  is_read: boolean;
  favorite: boolean;
  feed_name: string;
  source: string;
}
export interface Stats {
  feeds: number;
  active_feeds: number;
  entries: number;
  unread: number;
  favorites: number;
  downloads: number;
  storage_bytes: number;
  failed_jobs: number;
  pending_jobs: number;
}
export interface Settings {
  webhook_configured: boolean;
  downloads_enabled: boolean;
  notifications_enabled: boolean;
}
export interface Job {
  id: string;
  entry_id: string;
  kind: string;
  status: string;
  attempts: number;
  last_error: string | null;
  created_at: string;
  title: string;
  feed_name: string;
}
export interface Download {
  id: string;
  entry_id: string;
  filename: string;
  size_bytes: number;
  content_type: string;
  created_at: string;
  title: string;
  author: string;
}
export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message);
  }
}
export async function api<T>(path: string, method = 'GET', body?: unknown): Promise<T> {
  const response = await fetch(`/api${path}`, {
    method,
    credentials: 'same-origin',
    headers: body === undefined ? {} : { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: 'Unable to connect to the server' }));
    if (response.status === 401 && path !== '/login')
      window.dispatchEvent(new Event('session-expired'));
    throw new ApiError(error.error || 'Unable to complete the operation', response.status);
  }
  return response.json();
}
export const sources = [
  { id: 'rss', label: 'RSS / RSSHub', mark: '↗' },
  { id: 'pixiv', label: 'Pixiv', mark: 'p' },
  { id: 'bluesky', label: 'Bluesky', mark: 'B' },
  { id: 'xcancel', label: 'Xcancel', mark: '𝕏' },
  { id: 'pawchive', label: 'Pawchive', mark: '↟' },
];
export function date(value: string | null) {
  return value
    ? new Intl.DateTimeFormat('en-US', {
        day: '2-digit',
        month: 'short',
        hour: '2-digit',
        minute: '2-digit',
      }).format(new Date(value))
    : 'Not synced yet';
}
export function bytes(n: number) {
  return n < 1024
    ? `${n} B`
    : n < 1048576
      ? `${(n / 1024).toFixed(1)} KB`
      : n < 1073741824
        ? `${(n / 1048576).toFixed(1)} MB`
        : `${(n / 1073741824).toFixed(2)} GB`;
}
