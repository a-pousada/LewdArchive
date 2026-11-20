import { test, expect } from '@playwright/test';
import { buildFeedUrl } from '../src/feedTemplates';

test('RSSHub routes preserve the instance prefix and normalize artist input', () => {
  expect(buildFeedUrl('pawchive', 'http://rsshub:1200/', ' 22445 ', 'fanbox')).toBe(
    'http://rsshub:1200/pawchive/fanbox/22445',
  );
  expect(buildFeedUrl('pawchive', 'https://rsshub.app', '22445', 'patreon')).toBe(
    'https://rsshub.app/pawchive/patreon/22445',
  );
  expect(buildFeedUrl('pixiv', 'https://example.com/rsshub///', '15288095')).toBe(
    'https://example.com/rsshub/pixiv/user/15288095',
  );
  expect(buildFeedUrl('bluesky', 'https://rsshub.app', ' @Artist.bsky.social ')).toBe(
    'https://rsshub.app/bsky/profile/artist.bsky.social',
  );
});

test('template inputs cannot inject routes, parameters or credentials', () => {
  for (const id of ['', '../123', '12/34', '22445?limit=1', 'abc']) {
    expect(() => buildFeedUrl('pawchive', 'https://rsshub.app', id)).toThrow();
  }
  for (const base of [
    'invalid',
    'ftp://rsshub.app',
    'https://user:pass@rsshub.app',
    'https://rsshub.app?key=x',
    'https://rsshub.app/#fragment',
  ]) {
    expect(() => buildFeedUrl('pixiv', base, '123')).toThrow();
  }
  for (const handle of [
    'https://bsky.app/profile/artist.bsky.social',
    '../test',
    'artist',
    'artist.bsky.social?x=1',
  ]) {
    expect(() => buildFeedUrl('bluesky', 'https://rsshub.app', handle)).toThrow();
  }
});
