import { test, expect } from '@playwright/test';
test('personal archive: login, library, settings and feed lifecycle', async ({ page }, info) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Welcome to your archive.' })).toBeVisible();
  await page.getByLabel('Access key', { exact: true }).fill(process.env.API_TOKEN!);
  await page.getByRole('button', { name: 'Sign in to LewdArchive' }).click();
  await expect(page.getByRole('heading', { name: 'Your library.' })).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Every collection starts with an artist.' }),
  ).toBeVisible();
  await page.screenshot({
    path: `../.test-runtime/library-${info.project.name}.png`,
    fullPage: true,
  });
  await page.getByRole('button', { name: 'New feed', exact: true }).click();
  await page.getByLabel('Service', { exact: true }).selectOption('custom');
  await page.getByLabel('Feed name', { exact: true }).fill(`Browser test ${info.project.name}`);
  await page
    .getByLabel('Feed URL', { exact: true })
    .fill(`https://example.com/browser-test-${info.project.name}.rss`);
  await page.getByLabel('Follow this feed', { exact: false }).uncheck();
  await page.getByRole('dialog').getByRole('button', { name: 'Add feed', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  async function nav(label: string) {
    if (info.project.name === 'mobile')
      await page.getByRole('button', { name: 'Open menu' }).click();
    await page.locator('.sidebar').getByRole('button', { name: label, exact: true }).click();
  }
  await nav('Feeds 1');
  await expect(
    page.getByRole('heading', { name: `Browser test ${info.project.name}`, exact: true }),
  ).toBeVisible();
  await page
    .getByRole('button', { name: `Edit Browser test ${info.project.name}`, exact: true })
    .click();
  await page.getByLabel('Feed name', { exact: true }).fill(`Updated ${info.project.name}`);
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByRole('heading', { name: `Updated ${info.project.name}` })).toBeVisible();
  await page.getByRole('button', { name: `Delete Updated ${info.project.name}` }).click();
  await page.getByRole('button', { name: 'Remove feed', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Your artists, all in one place.' }),
  ).toBeVisible();
  await nav('Settings');
  await expect(page.getByRole('heading', { name: 'Connect to Discord' })).toBeVisible();
  await page.getByLabel('Allow downloads', { exact: false }).uncheck();
  await page.getByRole('button', { name: 'Save settings' }).click();
  await expect(page.getByRole('status')).toContainText('Settings saved.');
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Your library.' })).toBeVisible();
  await nav('Settings');
  await expect(page.getByLabel('Allow downloads', { exact: false })).not.toBeChecked();
  await page.getByLabel('Allow downloads', { exact: false }).check();
  await page.getByRole('button', { name: 'Save settings' }).click();
  await expect(page.getByRole('status')).toContainText('Settings saved.');
  await page.screenshot({
    path: `../.test-runtime/settings-${info.project.name}.png`,
    fullPage: true,
  });
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth,
  );
  expect(overflow).toBe(false);
  await nav('Downloads');
  await expect(page.getByRole('heading', { name: 'Your collection, offline too.' })).toBeVisible();
  await nav('Activity');
  await expect(page.getByRole('heading', { name: 'All quiet for now.' })).toBeVisible();
  await nav('Sign out');
  await expect(page.getByRole('heading', { name: 'Welcome to your archive.' })).toBeVisible();
  expect(errors).toEqual([]);
});

import http from 'node:http';
test('browse real feed entries, search, gallery, favorites and manual download', async ({
  page,
  request,
}, info) => {
  const fixturePort = 55889;
  const key = process.env.API_TOKEN!;
  let feedId: string | undefined;
  const fixture = http.createServer((req, res) => {
    if (req.url?.startsWith('/image.svg')) {
      res.writeHead(200, { 'Content-Type': 'image/svg+xml' });
      res.end(
        '<svg xmlns="http://www.w3.org/2000/svg" width="600" height="700" viewBox="0 0 600 700"><rect width="600" height="700" fill="#272334"/><circle cx="390" cy="210" r="100" fill="#d5a3b6"/><path d="M0 550 190 290 420 580 600 370V700H0Z" fill="#66556f"/><path d="M0 650 260 440 470 650 600 500V700H0Z" fill="#3c334b"/><text x="30" y="65" fill="#d5a3b6" font-family="sans-serif" font-size="20">LEWDARCHIVE · TEST FIXTURE</text></svg>',
      );
      return;
    }
    res.writeHead(200, { 'Content-Type': 'application/rss+xml' });
    res.end(
      `<rss version="2.0"><channel><title>Gallery fixture</title><link>https://example.com</link><description>Test only</description><item><guid>moon</guid><title>Moon study</title><author>Test Artist</author><link>https://example.com/moon</link><description><![CDATA[<p>A moonlit study from the test collection.</p><img src="http://127.0.0.1:${fixturePort}/image.svg"/><img src="http://127.0.0.1:${fixturePort}/image.svg?second"/>]]></description></item><item><guid>night</guid><title>Night sketch</title><link>https://example.com/night</link><description>Another drawing.</description></item></channel></rss>`,
    );
  });
  await new Promise<void>((resolve) => fixture.listen(fixturePort, '127.0.0.1', resolve));
  try {
    const added = await request.post('/api/feeds', {
      headers: { Authorization: `Bearer ${key}` },
      data: {
        name: `Gallery ${info.project.name}`,
        url: `http://127.0.0.1:${fixturePort}/feed`,
        source: 'pixiv',
        enabled: true,
        auto_download: false,
        notify: false,
        poll_minutes: 15,
      },
    });
    expect(added.status()).toBe(201);
    feedId = (await added.json()).id;
    await expect
      .poll(async () => {
        const r = await request.get(`/api/entries?feed_id=${feedId}`, {
          headers: { Authorization: `Bearer ${key}` },
        });
        return (await r.json()).total;
      })
      .toBe(2);
    await page.goto('/');
    await page.getByLabel('Access key', { exact: true }).fill(key);
    await page.getByRole('button', { name: 'Sign in to LewdArchive' }).click();
    await expect(page.getByRole('button', { name: 'Moon study', exact: true })).toBeVisible();
    await page.screenshot({
      path: `../.test-runtime/gallery-${info.project.name}.png`,
      fullPage: true,
    });
    await page.getByRole('button', { name: 'Moon study', exact: true }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByRole('heading', { name: 'Moon study' })).toBeVisible();
    await expect(dialog.locator('.modal-head')).toContainText('Test Artist');
    await dialog.getByRole('button', { name: 'Test on Discord', exact: true }).click();
    await expect(dialog.getByRole('status')).toContainText('Configure the Discord webhook');
    // Exercise queued feedback without sending a message to any external service.
    await page.route('**/api/entries/*/discord', (route) =>
      route.fulfill({
        status: 202,
        contentType: 'application/json',
        body: JSON.stringify({ ok: true, job_id: 'test-job' }),
      }),
    );
    await dialog.getByRole('button', { name: 'Test on Discord', exact: true }).click();
    await expect(dialog.getByRole('status')).toContainText('Discord test queued');
    await page.unroute('**/api/entries/*/discord');
    await dialog.getByRole('button', { name: 'Next media' }).click();
    await expect(dialog.locator('.media-navigation')).toContainText('2 / 2');
    await dialog.getByRole('button', { name: 'Favorite', exact: true }).click();
    await expect(dialog.getByRole('button', { name: 'Favorited', exact: true })).toBeVisible();
    await dialog.getByRole('button', { name: 'Download files', exact: true }).click();
    await expect(page.locator('.toast')).toContainText('Download queued');
    await dialog.getByRole('button', { name: 'Close', exact: true }).click();
    await page.getByLabel('Search posts or artists').fill('nothing matches');
    await expect(page.getByRole('heading', { name: 'No posts here yet.' })).toBeVisible();
    await page.getByLabel('Search posts or artists').fill('Moon');
    await expect(page.getByRole('button', { name: 'Moon study', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Night sketch', exact: true })).toHaveCount(0);
    if (info.project.name === 'mobile')
      await page.getByRole('button', { name: 'Open menu' }).click();
    await page
      .locator('.sidebar')
      .getByRole('button', { name: /^Favorites/ })
      .click();
    await expect(page.getByRole('button', { name: 'Moon study', exact: true })).toBeVisible();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth),
    ).toBe(false);
  } finally {
    if (feedId)
      await request.delete(`/api/feeds/${feedId}`, { headers: { Authorization: `Bearer ${key}` } });
    await new Promise<void>((resolve) => fixture.close(() => resolve()));
  }
});
