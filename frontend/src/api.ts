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

export interface Webhook {
  category_filter: string | null;
  category_filters?: string[] | null;
  include_tags?: string[];
  exclude_tags?: string[];
  id: string;
  name: string;
  username: string;
  avatar_url: string;
  embed_color: number;
  enabled: boolean;
  created_at: string;
}
