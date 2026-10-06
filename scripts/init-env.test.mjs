import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

test('environment setup generates consistent secrets and never overwrites an existing file', () => {
  const directory = mkdtempSync(join(tmpdir(), 'lewdarchive-env-'));
  try {
    mkdirSync(join(directory, 'scripts'));
    copyFileSync(
      new URL('./init-env.mjs', import.meta.url),
      join(directory, 'scripts/init-env.mjs'),
    );
    copyFileSync(new URL('../.env.example', import.meta.url), join(directory, '.env.example'));
    const run = () =>
      spawnSync(process.execPath, [join(directory, 'scripts/init-env.mjs')], {
        cwd: tmpdir(),
        encoding: 'utf8',
      });
    const first = run();
    assert.equal(first.status, 0, first.stderr);
    const configuration = readFileSync(join(directory, '.env'), 'utf8');
    const values = Object.fromEntries(
      configuration
        .split('\n')
        .filter((line) => /^[A-Z_]+=/.test(line))
        .map((line) => [line.slice(0, line.indexOf('=')), line.slice(line.indexOf('=') + 1)]),
    );
    assert.match(values.API_TOKEN, /^[a-f0-9]{64}$/);
    assert.match(values.POSTGRES_PASSWORD, /^[a-f0-9]{64}$/);
    assert.notEqual(values.API_TOKEN, values.POSTGRES_PASSWORD);
    assert.equal(new URL(values.DATABASE_URL).password, values.POSTGRES_PASSWORD);
    assert.equal(values.RSSHUB_BASE_URL, '');
    assert.equal(values.RUST_LOG, 'lewdarchive=info,tower_http=info');
    assert(!first.stdout.includes(values.API_TOKEN));
    assert(!first.stdout.includes(values.POSTGRES_PASSWORD));
    if (process.platform !== 'win32')
      assert.equal(statSync(join(directory, '.env')).mode & 0o777, 0o600);
    const second = run();
    assert.equal(second.status, 1);
    assert.match(second.stderr, /already exists/);
    assert.equal(readFileSync(join(directory, '.env'), 'utf8'), configuration);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
