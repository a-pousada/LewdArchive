import { test, expect } from '@playwright/test';

test('review AI suggestions, persist tags and handle provider errors', async ({ page }, info) => {
  const post = { id: 'p', artist_id: 'a', title: 'Evening studies', author: 'Aoi', feed_name: 'Aoi', category: '2D Illustration', tags: ['landscape'], summary: 'A study of evening light.', url: 'https://example.com/original', media: [{kind:'image',url:'https://example.com/fixture.png'}], published_at: '2026-10-19T12:00:00Z', liked: false, favorite: false, likes: 2 };
  await page.route('https://example.com/fixture.png', route => route.fulfill({contentType:'image/svg+xml',body:'<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><rect width="32" height="16" fill="blue"/></svg>'}));
  let saves = 0, suggestions = 0;
  await page.route('**/api/community/**', async route => {
    const path = new URL(route.request().url()).pathname;
    let json: unknown = {};
    if (path.endsWith('/config')) json = { discord_enabled: true };
    else if (path.endsWith('/me')) json = { id: 'u', display_name: 'Collector' };
    else if (path.endsWith('/servers')) json = [{ id: 's', name: 'Atelier', connected: true, external_id: '1' }];
    else if (path.endsWith('/posts/p')) json = { post, can_manage: true };
    else if (path.endsWith('/ai')) json = { enabled: true, images: true, provider: 'wd', model: 'SmilingWolf/wd-swinv2-tagger-v3' };
    else if (path.endsWith('/suggest')) {
      suggestions++;
      expect(route.request().postDataJSON()).toEqual({});
      if (suggestions === 2) { await route.fulfill({ status: 502, json: { error: 'AI provider is unavailable' } }); return; }
      json = { tags: ['digital_art', 'blue_sky'], saved: false };
    } else if (path.endsWith('/tags')) { saves++; post.tags = route.request().postDataJSON().tags; json = { tags: post.tags }; }
    else if (path.endsWith('/comments')) json = { items: [], total: 0, page: 1, pages: 0 };
    await route.fulfill({ json });
  });
  await page.goto('/community?server=s&post=p');
  await page.getByText('Edit tags and AI suggestions').click();
  await page.getByRole('button', { name: 'Suggest with AI' }).click();
  await expect(page.getByText('Review suggestions')).toBeVisible();
  expect(saves).toBe(0);
  await page.getByRole('button', { name: 'Remove suggestion blue_sky' }).click();
  await page.getByRole('button', { name: 'Add to draft' }).click();
  await expect(page.getByRole('textbox', { name: 'Post tags', exact: true })).toHaveValue('landscape, digital_art');
  await page.getByRole('button', { name: 'Save tags', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Tags saved.');
  expect(saves).toBe(1);
  await page.getByRole('button', { name: 'Suggest with AI' }).click();
  await expect(page.getByRole('alert')).toHaveText('AI provider is unavailable');
  await expect(page.getByRole('textbox', { name: 'Post tags', exact: true })).toHaveValue('landscape, digital_art');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
  await page.screenshot({ path: `../.test-runtime/tags-${info.project.name}.png`, fullPage: true });
});

test('WD requires images while manual tags remain available', async ({ page }) => {
  const post = { id: 'p', artist_id: 'a', title: 'A text-only update', author: 'Aoi', feed_name: 'Aoi', tags: [], summary: 'Studio news.', url: 'https://example.com/original', media: [], published_at: '2026-10-19T12:00:00Z', likes: 0 };
  await page.route('**/api/community/**', async route => {
    const path = new URL(route.request().url()).pathname;
    let json: unknown = {};
    if (path.endsWith('/config')) json = { discord_enabled: true };
    else if (path.endsWith('/me')) json = { id: 'u', display_name: 'Collector' };
    else if (path.endsWith('/servers')) json = [{ id: 's', name: 'Atelier', connected: true, external_id: '1' }];
    else if (path.endsWith('/posts/p')) json = { post, can_manage: true };
    else if (path.endsWith('/ai')) json = { enabled: true, model: 'SmilingWolf/wd-swinv2-tagger-v3' };
    else if (path.endsWith('/comments')) json = { items: [], total: 0, page: 1, pages: 0 };
    else if (path.endsWith('/tags')) json = { tags: route.request().postDataJSON().tags };
    else if (path.endsWith('/suggest')) throw new Error('Text-only posts must not request WD inference');
    await route.fulfill({ json });
  });
  await page.goto('/community?server=s&post=p');
  await page.getByText('Edit tags and AI suggestions').click();
  await expect(page.getByRole('button', { name: 'Suggest with AI' })).toBeDisabled();
  await expect(page.getByText('This post has no images to analyze.', { exact: false })).toBeVisible();
  await page.getByRole('textbox', { name: 'Post tags', exact: true }).fill('studio_news');
  await page.getByRole('button', { name: 'Save tags', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Tags saved.');
});

test('category filters, artist profiles and channel configuration', async ({ page }, info) => {
  const artist = { id: 'a', name: 'Aoi', category: '2D Illustration', source: 'rss', url: 'https://example.com/rss', avatar_url: '', official_links: [], enabled: true, favorite: false, notify: false };
  const categories = [{ category: '2D Illustration', channel_id: null as string | null, channel_name: '', artists: 1 }, { category: '3D Art', channel_id: null, channel_name: '', artists: 0 }];
  const updates: unknown[] = [];
  let filterSeen = false;
  await page.route('**/api/community/**', async route => {
    const url = new URL(route.request().url()), path = url.pathname;
    let json: unknown = {};
    if (path.endsWith('/config')) json = { discord_enabled: true };
    else if (path.endsWith('/me')) json = { id: 'u', display_name: 'Collector' };
    else if (path.endsWith('/servers')) json = [{ id: 's', name: 'Atelier', connected: true, external_id: '1' }];
    else if (path.endsWith('/servers/s')) json = { server: { id: 's', name: 'Atelier', channel_id: '123', channel_name: 'artwork', enabled: true, mentions_enabled: true }, can_manage: true };
    else if (path.endsWith('/categories')) {
      if (route.request().method() === 'PUT') { const data = route.request().postDataJSON(); updates.push(data); categories[0].channel_id = data.channel_id; categories[0].channel_name = 'illustrations'; json = { ok: true }; } else json = categories;
    } else if (path.endsWith('/posts')) { filterSeen ||= url.searchParams.get('category') === '2D Illustration'; json = { items: [], total: 0, page: 1, pages: 0 }; }
    else if (path.endsWith('/artists')) json = [artist];
    else if (path.endsWith('/profile')) { Object.assign(artist, route.request().postDataJSON()); json = { ok: true }; }
    else if (path.endsWith('/webhooks')) {
      if (route.request().method() === 'POST') { updates.push(route.request().postDataJSON()); json = { id: 'w' }; } else json = [];
    }
    await route.fulfill({ json });
  });
  await page.goto('/community?server=s');
  await page.getByLabel('Category', { exact: true }).selectOption('2D Illustration');
  await expect.poll(() => filterSeen).toBe(true);
  await page.getByRole('button', { name: 'Artists', exact: true }).click();
  await page.getByRole('button', { name: 'Edit profile' }).click();
  await page.getByLabel('Artist category').fill('3D Art');
  await page.getByRole('button', { name: 'Save profile' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  expect(artist.category).toBe('3D Art');
  await page.getByRole('button', { name: 'Server settings', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Category channels' })).toBeVisible();
  await page.getByRole('button', { name: /2D Illustration.*Default channel/ }).click();
  await page.getByLabel('Category channel ID').fill('456789');
  await page.getByRole('button', { name: 'Save category channel' }).click();
  await expect(page.getByText('Category channel saved.')).toBeVisible();
  expect(updates[0]).toEqual({ category: '2D Illustration', channel_id: '456789' });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
  await page.screenshot({ path: `../.test-runtime/categories-${info.project.name}.png`, fullPage: true });
  await page.getByRole('button', { name: 'Add webhook', exact: true }).click();
  await page.getByLabel('Destination name').fill('3D gallery');
  await page.getByLabel('Webhook URL').fill('https://example.com/hook');
  await page.getByLabel('Category filter').selectOption('3D Art');
  await page.getByRole('button', { name: 'Save webhook', exact: true }).click();
  await expect.poll(() => updates.length).toBe(2);
  expect(updates[1]).toMatchObject({ category_filter: '3D Art' });
});
