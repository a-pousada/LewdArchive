import assert from 'node:assert/strict';
import http from 'node:http';
import { setTimeout as wait } from 'node:timers/promises';
const base = process.env.TEST_BASE_URL || 'http://127.0.0.1:8080';
const token = process.env.API_TOKEN;
assert(token, 'Set API_TOKEN to the test server key');
const fixturePort = Number(process.env.FIXTURE_PORT || 55888);
let version = 1;
let conditionalRequests = 0;
let feedId;
const fixture = http.createServer((req, res) => {
  if (req.url != '/feed') {
    res.writeHead(404).end();
    return;
  }
  if (req.headers['if-none-match'] === `"v${version}"`) {
    conditionalRequests++;
    res.writeHead(304).end();
    return;
  }
  const item = (n) =>
    `<item><guid>fixture-${n}</guid><title>Drawing ${n}</title><author>Aoi</author><link>https://example.com/art/${n}</link><description><![CDATA[<p>A safe test drawing.</p><img src="${n === 3 && process.env.TEST_PUBLIC_MEDIA_URL ? process.env.TEST_PUBLIC_MEDIA_URL : `http://127.0.0.1:${fixturePort}/blocked.png`}"/>]]></description><pubDate>${new Date(Date.now() + n * 1000).toUTCString()}</pubDate></item>`;
  res.writeHead(200, { 'Content-Type': 'application/rss+xml', ETag: `"v${version}"` });
  res.end(
    `<rss version="2.0"><channel><title>Test Artist</title><link>https://example.com</link><description>Integration fixture</description>${Array.from({ length: version }, (_, i) => item(i + 1)).join('')}</channel></rss>`,
  );
});
await new Promise((resolve) => fixture.listen(fixturePort, '127.0.0.1', resolve));
async function request(path, method = 'GET', body, expected = 200) {
  const r = await fetch(`${base}/api${path}`, {
    method,
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const data = await r.json();
  assert.equal(r.status, expected, JSON.stringify(data));
  return data;
}
async function until(fn, message) {
  for (let i = 0; i < 60; i++) {
    if (await fn()) return;
    await wait(500);
  }
  throw Error(message);
}
try {
  const unauth = await fetch(`${base}/api/feeds`);
  assert.equal(unauth.status, 401);
  const badLogin = await fetch(`${base}/api/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ token: 'incorrect' }),
  });
  assert.equal(badLogin.status, 401);
  const login = await fetch(`${base}/api/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ token }),
  });
  assert.equal(login.status, 200);
  assert.match(login.headers.get('set-cookie'), /HttpOnly; SameSite=Strict/);
  await request('/settings', 'PUT', { downloads_enabled: true, notifications_enabled: false });
  const input = {
    name: 'Integration Artist',
    url: `http://127.0.0.1:${fixturePort}/feed`,
    source: 'pixiv',
    enabled: true,
    auto_download: true,
    notify: true,
    poll_minutes: 5,
  };
  await request('/feeds', 'POST', { ...input, poll_minutes: 0 }, 400);
  const f = await request('/feeds', 'POST', input, 201);
  feedId = f.id;
  await request('/feeds', 'POST', input, 409);
  await until(
    async () => (await request('/feeds')).find((f) => f.id === feedId)?.initialized,
    'Feed did not initialize',
  );
  let entries = await request(`/entries?feed_id=${feedId}`);
  assert.equal(entries.total, 1);
  let jobs = (await request('/jobs')).filter((j) => j.entry_id === entries.items[0].id);
  assert.equal(jobs.length, 0, 'Historical entries must not notify or download');
  await request(`/feeds/${feedId}/sync`, 'POST');
  await until(() => conditionalRequests > 0, 'ETag was not reused');
  version = 2;
  await request(`/feeds/${feedId}/sync`, 'POST');
  await until(
    async () => (await request(`/entries?feed_id=${feedId}`)).total === 2,
    'New entry not imported',
  );
  entries = await request(`/entries?feed_id=${feedId}`);
  const entry = entries.items.find((e) => e.title === 'Drawing 2');
  assert(entry);
  await until(async () => {
    jobs = (await request('/jobs')).filter((j) => j.entry_id === entry.id);
    return jobs.length === 2 && jobs.some((j) => j.kind === 'download' && j.last_error);
  }, 'Jobs not processed');
  assert.equal(
    jobs.find((j) => j.kind === 'discord').status,
    'pending',
    'Disabled notifications must stay queued',
  );
  assert.match(
    jobs.find((j) => j.kind === 'download').last_error,
    /Private|reserved/,
    'Private media hosts must be rejected',
  );
  await request(`/entries/${entry.id}`, 'PATCH', { favorite: true, is_read: true });
  assert.equal((await request(`/entries?feed_id=${feedId}&favorite=true`)).total, 1);
  assert.equal((await request(`/entries?feed_id=${feedId}&unread=true`)).total, 1);
  assert.equal((await request(`/entries?feed_id=${feedId}&q=Drawing%202`)).total, 1);
  await request(`/feeds/${feedId}/sync`, 'POST');
  await wait(1500);
  assert.equal(
    (await request(`/entries?feed_id=${feedId}`)).total,
    2,
    'Duplicate posts must not be inserted',
  );
  if (process.env.TEST_PUBLIC_MEDIA_URL) {
    version = 3;
    await request(`/feeds/${feedId}/sync`, 'POST');
    await until(
      async () => (await request(`/entries?feed_id=${feedId}`)).total === 3,
      'Public media entry missing',
    );
    const publicEntry = (await request(`/entries?feed_id=${feedId}`)).items.find(
      (e) => e.title === 'Drawing 3',
    );
    let file;
    await until(async () => {
      file = (await request('/downloads')).find((d) => d.entry_id === publicEntry.id);
      return !!file;
    }, 'Public media download failed');
    const response = await fetch(`${base}/api/downloads/${file.id}/file`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    assert.equal(response.status, 200);
    assert.match(response.headers.get('content-disposition'), /attachment/);
    assert.equal((await response.arrayBuffer()).byteLength, file.size_bytes);
    assert(file.size_bytes > 0);
    await request(`/entries/${publicEntry.id}/download`, 'POST');
    await until(
      async () =>
        (await request('/jobs')).some(
          (j) => j.entry_id === publicEntry.id && j.kind === 'download' && j.status === 'done',
        ),
      'Manual download did not complete',
    );
    assert.equal(
      (await request('/downloads')).filter((d) => d.entry_id === publicEntry.id).length,
      1,
      'Duplicate file created',
    );
    console.log(
      'PASS: public media download, authenticated file serving, byte count and download idempotency',
    );
  }
  await request(`/feeds/${feedId}`, 'PUT', { ...input, enabled: false });
  await request(`/feeds/${feedId}/sync`, 'POST', undefined, 400);
  await request(
    '/settings',
    'PUT',
    {
      downloads_enabled: true,
      notifications_enabled: true,
      webhook_url: 'https://example.com/not-discord',
    },
    400,
  );
  const settings = await request('/settings');
  assert(!('webhook_url' in settings), 'Webhook secrets must not be returned');
  console.log(
    'PASS: auth, cookies, validation, CRUD, first sync, ETag, deduplication, outbox, pause, SSRF, favorites, read status, search, secret redaction',
  );
} finally {
  if (feedId) await request(`/feeds/${feedId}`, 'DELETE');
  await request('/settings', 'PUT', { downloads_enabled: true, notifications_enabled: true });
  await new Promise((resolve) => fixture.close(resolve));
}
