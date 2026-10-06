export type FeedTemplate = 'pawchive' | 'pixiv' | 'bluesky' | 'custom';
export type PawchiveService = 'fanbox' | 'patreon';

export const feedTemplates = [
  { id: 'pawchive', label: 'Pawchive' },
  { id: 'pixiv', label: 'Pixiv' },
  { id: 'bluesky', label: 'Bluesky' },
  { id: 'custom', label: 'Custom — full URL' },
] as const;

// RSSHub route references: docs/feed-templates.md.
export function buildFeedUrl(
  template: Exclude<FeedTemplate, 'custom'>,
  baseUrl: string,
  identifier: string,
  service: PawchiveService = 'fanbox',
): string {
  let base: URL;
  try {
    base = new URL(baseUrl.trim());
  } catch {
    throw new Error('Enter a valid URL for the RSSHub instance.');
  }
  if (
    !['http:', 'https:'].includes(base.protocol) ||
    base.username ||
    base.password ||
    base.search ||
    base.hash
  ) {
    throw new Error(
      'Use the RSSHub HTTP(S) base URL without credentials, query parameters, or fragments.',
    );
  }
  const id = identifier.trim();
  if (!id) throw new Error('Enter the artist identifier.');
  let route: string;
  if (template === 'bluesky') {
    const handle = id.replace(/^@/, '').toLowerCase();
    if (
      handle.length > 253 ||
      !/^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(
        handle,
      )
    ) {
      throw new Error(
        'Enter the full handle, such as artist.bsky.social, without the profile URL.',
      );
    }
    route = `bsky/profile/${encodeURIComponent(handle)}`;
  } else {
    if (!/^[0-9]+$/.test(id)) throw new Error('The artist ID must contain only numbers.');
    if (template === 'pawchive' && !['fanbox', 'patreon'].includes(service)) {
      throw new Error('Select Fanbox or Patreon.');
    }
    route = template === 'pawchive' ? `pawchive/${service}/${id}` : `pixiv/user/${id}`;
  }
  return `${base.href.replace(/\/+$/, '')}/${route}`;
}
