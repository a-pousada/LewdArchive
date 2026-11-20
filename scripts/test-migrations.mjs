import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
const db = process.env.DATABASE_URL;
assert(db && new URL(db).pathname.endsWith('_test'), 'Use a dedicated *_test database');
const initial = readFileSync(
  new URL('../backend/migrations/0001_init.sql', import.meta.url),
  'utf8',
);
const upgrade = readFileSync(
  new URL('../backend/migrations/0002_webhooks_and_feed_transfer.sql', import.meta.url),
  'utf8',
);
const communities = readFileSync(new URL("../backend/migrations/0003_communities.sql",import.meta.url),"utf8");
const trending = readFileSync(new URL("../backend/migrations/0004_trending_artist_profiles.sql",import.meta.url),"utf8");
const profiles = readFileSync(new URL("../backend/migrations/0005_feed_artist_profiles.sql",import.meta.url),"utf8");
const communityTools = readFileSync(new URL("../backend/migrations/0006_community_webhooks_comments.sql",import.meta.url),"utf8");
const webhookScopes = readFileSync(new URL("../backend/migrations/0007_webhook_scope_uniqueness.sql",import.meta.url),"utf8");
const serverAutomation = readFileSync(new URL("../backend/migrations/0008_server_owned_automation.sql",import.meta.url),"utf8");
const taxonomy = readFileSync(new URL("../backend/migrations/0009_categories_and_tags.sql",import.meta.url),"utf8");
const automaticTags = readFileSync(new URL("../backend/migrations/0010_automatic_tagging.sql",import.meta.url),"utf8");
for (const configured of [false, true]) {
  const sql = `BEGIN;
    CREATE SCHEMA migration_fixture;
    SET LOCAL search_path TO migration_fixture;
    ${initial}
    ${configured ? "UPDATE settings SET webhook_url='https://discord.com/api/webhooks/0/test-only-never-send-0000000000';" : ''}
    INSERT INTO feeds(id,name,url) VALUES('11111111-1111-1111-1111-111111111111','Legacy artist','https://example.com/rss');
    INSERT INTO entries(id,feed_id,guid,title,author,url,published_at) VALUES('22222222-2222-2222-2222-222222222222','11111111-1111-1111-1111-111111111111','one','Post','Artist','https://example.com/post',now());
    INSERT INTO jobs(id,entry_id,kind,status,attempts,last_error) VALUES('33333333-3333-3333-3333-333333333333','22222222-2222-2222-2222-222222222222','discord','failed',8,'Previous error');
    ${upgrade}
    ${communities}
    ${trending}
    ${profiles}
    ${communityTools}
    ${webhookScopes}
    ${serverAutomation}
    ${taxonomy}
    ${automaticTags}
    DO $$ BEGIN
      IF (SELECT count(*) FROM webhooks) <> ${configured ? 1 : 0} THEN RAISE EXCEPTION 'Destination migration failed'; END IF;
      IF NOT EXISTS(SELECT 1 FROM jobs WHERE status='failed' AND attempts=8 AND last_error='Previous error' AND webhook_id IS ${configured ? 'NOT ' : ''}NULL) THEN RAISE EXCEPTION 'Job state was lost'; END IF;
      IF (SELECT count(*) FROM feeds) <> 1 OR (SELECT count(*) FROM entries) <> 1 THEN RAISE EXCEPTION 'Feed data was lost'; END IF;
    END $$;
    ROLLBACK;`;
  execFileSync(process.env.PSQL_PATH || 'psql', [db, '-X', '-v', 'ON_ERROR_STOP=1'], {
    input: sql,
    stdio: ['pipe', 'pipe', 'pipe'],
  });
}
console.log(
  'PASS: upgrade preserves legacy destination, failed delivery state, feeds, entries, and unconfigured installations',
);
