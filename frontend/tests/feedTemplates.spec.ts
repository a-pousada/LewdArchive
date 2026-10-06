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

test('create feeds from presets and preserve custom URLs and existing feeds', async ({
  page,
  request,
}, info) => {
  const headers = { Authorization: `Bearer ${process.env.API_TOKEN!}` };
  const options = await request.get('/api/feed-options', { headers });
  expect(options.ok()).toBe(true);
  const { rsshub_base_url: base } = await options.json();
  const ids: string[] = [];
  try {
    await page.goto('/');
    await page.getByLabel('Access key', { exact: true }).fill(process.env.API_TOKEN!);
    await page.getByRole('button', { name: 'Sign in to LewdArchive' }).click();
    const presets = [
      { template: 'pawchive', id: '22445', platform: 'fanbox', path: '/pawchive/fanbox/22445' },
      { template: 'pawchive', id: '22445', platform: 'patreon', path: '/pawchive/patreon/22445' },
      { template: 'pixiv', id: '15288095', path: '/pixiv/user/15288095' },
      { template: 'bluesky', id: '@artist.bsky.social', path: '/bsky/profile/artist.bsky.social' },
    ];
    for (const [index, preset] of presets.entries()) {
      await page.getByRole('button', { name: 'New feed', exact: true }).click();
      const dialog = page.getByRole('dialog');
      const name = `Template ${info.project.name} ${index}`;
      await dialog.getByLabel('Feed name', { exact: true }).fill(name);
      await dialog.getByLabel('Service', { exact: true }).selectOption(preset.template);
      await expect(dialog.getByLabel('RSSHub instance', { exact: true })).toHaveValue(base);
      if (preset.platform)
        await dialog.getByLabel('Pawchive platform').selectOption(preset.platform);
      await expect(dialog.getByRole('button', { name: 'Add feed', exact: true })).toBeDisabled();
      await dialog
        .getByLabel(preset.template === 'bluesky' ? 'Artist handle' : 'Artist ID', {
          exact: true,
        })
        .fill(preset.id);
      await expect(dialog.locator('.feed-url-preview code')).toHaveText(`${base}${preset.path}`);
      await dialog.getByLabel('Follow this feed', { exact: true }).uncheck();
      if (index === 0) {
        await dialog.getByLabel('Artist ID', { exact: true }).fill('../22445');
        await expect(dialog.getByRole('button', { name: 'Add feed', exact: true })).toBeDisabled();
        await dialog.getByLabel('Artist ID', { exact: true }).fill(preset.id);
        await page.screenshot({
          path: `../.test-runtime/feed-builder-${info.project.name}.png`,
          fullPage: true,
        });
      }
      if (index === 2) {
        await dialog
          .getByLabel('RSSHub instance', { exact: true })
          .fill('https://example.com/rsshub/');
        await expect(dialog.locator('.feed-url-preview code')).toHaveText(
          `https://example.com/rsshub${preset.path}`,
        );
      }
      await dialog.getByRole('button', { name: 'Add feed', exact: true }).click();
      await expect(dialog).toHaveCount(0);
      const feed = (await (await request.get('/api/feeds', { headers })).json()).find(
        (f: { name: string }) => f.name === name,
      );
      expect(feed).toBeTruthy();
      ids.push(feed.id);
      expect(feed.url).toBe(`${index === 2 ? 'https://example.com/rsshub' : base}${preset.path}`);
      expect(feed.source).toBe(preset.template);
      expect(feed.enabled).toBe(false);
    }
    await page.getByRole('button', { name: 'New feed', exact: true }).click();
    const dialog = page.getByRole('dialog');
    await dialog.getByLabel('Feed name', { exact: true }).fill(`Custom ${info.project.name}`);
    await dialog.getByLabel('Service', { exact: true }).selectOption('custom');
    const customUrl = `https://example.com/custom-${info.project.name}.xml?limit=5&category=art`;
    await dialog.getByLabel('Feed URL', { exact: true }).fill(customUrl);
    await dialog.getByLabel('Source', { exact: true }).selectOption('xcancel');
    await dialog.getByLabel('Service', { exact: true }).selectOption('pawchive');
    await expect(dialog.getByRole('button', { name: 'Add feed', exact: true })).toBeDisabled();
    await dialog.getByLabel('Service', { exact: true }).selectOption('custom');
    await expect(dialog.getByLabel('Feed URL', { exact: true })).toHaveValue(customUrl);
    await dialog.getByLabel('Follow this feed', { exact: true }).uncheck();
    await dialog.getByRole('button', { name: 'Add feed', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    const custom = (await (await request.get('/api/feeds', { headers })).json()).find(
      (f: { url: string }) => f.url === customUrl,
    );
    expect(custom).toBeTruthy();
    ids.push(custom.id);
    expect(custom.source).toBe('xcancel');
    if (info.project.name === 'mobile')
      await page.getByRole('button', { name: 'Open menu' }).click();
    await page
      .locator('.sidebar')
      .getByRole('button', { name: /^Feeds/ })
      .click();
    await page
      .getByRole('button', { name: `Edit Template ${info.project.name} 0`, exact: true })
      .click();
    await expect(page.getByRole('dialog').getByLabel('Feed URL', { exact: true })).toHaveValue(
      `${base}/pawchive/fanbox/22445`,
    );
    await expect(page.getByRole('dialog').getByLabel('Feed URL', { exact: true })).toHaveAttribute(
      'readonly',
      '',
    );
    await page.getByRole('dialog').getByLabel('Feed name', { exact: true }).fill('Renamed preset');
    await page.getByRole('button', { name: 'Save changes', exact: true }).click();
    await expect(page.getByRole('dialog')).toHaveCount(0);
    const updated = (await (await request.get('/api/feeds', { headers })).json()).find(
      (f: { id: string }) => f.id === ids[0],
    );
    expect(updated.url).toBe(`${base}/pawchive/fanbox/22445`);
    expect(updated.name).toBe('Renamed preset');
  } finally {
    for (const id of ids) await request.delete(`/api/feeds/${id}`, { headers });
  }
});
