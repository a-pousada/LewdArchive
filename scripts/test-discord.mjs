// Run only against an isolated test database. The scheduler lock prevents external sends.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
const db = process.env.DATABASE_URL;
assert(
  db && new URL(db).pathname.endsWith('_test'),
  'DATABASE_URL must point to a dedicated *_test database',
);
assert(process.env.API_TOKEN, 'API_TOKEN is required');
const psql = process.env.PSQL_PATH || 'psql';
const sql = (query) =>
  execFileSync(psql, [db, '-X', '-tA', '-v', 'ON_ERROR_STOP=1', '-c', query], {
    encoding: 'utf8',
  }).trim();
assert.equal(sql('SELECT count(*) FROM feeds'), '0', 'Test database must contain no feeds');
assert.equal(
  sql('SELECT webhook_url FROM settings'),
  '',
  'Test database must not contain a real webhook',
);
const base = process.env.TEST_BASE_URL || 'http://127.0.0.1:8080';
const headers = {
  Authorization: `Bearer ${process.env.API_TOKEN}`,
  'Content-Type': 'application/json',
};
const feed = randomUUID(),
  entry = randomUUID();
const lock = spawn(psql, [db, '-X', '-tA', '-v', 'ON_ERROR_STOP=1'], {
  stdio: ['pipe', 'pipe', 'pipe'],
});
await new Promise((resolve, reject) => {
  let output = '';
  lock.stdout.on('data', (data) => {
    output += data;
    if (output.includes('LOCKED')) resolve();
  });
  lock.once('error', reject);
  lock.once('exit', (code) => reject(Error(`Lock process exited: ${code}`)));
  lock.stdin.write('SELECT pg_advisory_lock(73421901);\n\\echo LOCKED\n');
});
async function send(expected) {
  const res = await fetch(`${base}/api/entries/${entry}/discord`, { method: 'POST', headers });
  const body = await res.json();
  assert.equal(res.status, expected, JSON.stringify(body));
  return body;
}
try {
  sql(
    `INSERT INTO feeds(id,name,url,enabled,notify) VALUES('${feed}','Webhook fixture','https://example.com/discord-test',false,false); INSERT INTO entries(id,feed_id,guid,title,author,url,published_at) VALUES('${entry}','${feed}','one','Test post','Test artist','https://example.com/post',now());`,
  );
  const unauthorized = await fetch(`${base}/api/entries/${entry}/discord`, { method: 'POST' });
  assert.equal(unauthorized.status, 401);
  assert.match((await send(400)).error, /Configure the Discord webhook/);
  // The invalid webhook is never contacted: this process holds the worker's advisory lock.
  sql(
    "UPDATE settings SET webhook_url='https://discord.com/api/webhooks/0/test-only-never-send-0000000000', notifications_enabled=false",
  );
  assert.match((await send(400)).error, /paused/);
  sql('UPDATE settings SET notifications_enabled=true');
  const missing = await fetch(`${base}/api/entries/${randomUUID()}/discord`, {
    method: 'POST',
    headers,
  });
  assert.equal(missing.status, 404);
  const results = await Promise.all([
    fetch(`${base}/api/entries/${entry}/discord`, { method: 'POST', headers }),
    fetch(`${base}/api/entries/${entry}/discord`, { method: 'POST', headers }),
  ]);
  assert.deepEqual(results.map((r) => r.status).sort(), [202, 409]);
  assert.equal(sql(`SELECT count(*) FROM jobs WHERE entry_id='${entry}' AND kind='discord'`), '1');
  assert.equal(sql(`SELECT status FROM jobs WHERE entry_id='${entry}'`), 'pending');
  const job = sql(`SELECT id FROM jobs WHERE entry_id='${entry}'`);
  sql(`UPDATE jobs SET status='processing' WHERE id='${job}'`);
  await send(409);
  for (const state of ['done', 'failed']) {
    sql(
      `UPDATE jobs SET status='${state}',attempts=8,last_error='previous failure',locked_at=now() WHERE id='${job}'`,
    );
    assert.equal((await send(202)).job_id, job);
    assert.equal(
      sql(
        `SELECT status || ':' || attempts || ':' || (last_error IS NULL AND locked_at IS NULL)::text FROM jobs WHERE id='${job}'`,
      ),
      'pending:0:true',
    );
  }
  console.log(
    'PASS: manual Discord queue, missing webhook, pause, authentication, missing post, concurrent deduplication, processing guard, resend and reset; no external messages sent',
  );
} finally {
  sql(
    `DELETE FROM feeds WHERE id='${feed}'; UPDATE settings SET webhook_url='',notifications_enabled=true;`,
  );
  lock.stdin.end('SELECT pg_advisory_unlock(73421901);\n\\q\n');
}
