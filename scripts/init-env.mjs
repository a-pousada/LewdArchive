import { randomBytes } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';

const password = randomBytes(32).toString('hex');
const template = readFileSync(new URL('../.env.example', import.meta.url), 'utf8');
const values = {
  API_TOKEN: randomBytes(32).toString('hex'),
  POSTGRES_PASSWORD: password,
  DATABASE_URL: `postgres://lewdarchive:${password}@localhost:5432/lewdarchive`,
};
const configuration = template.replace(
  /^(API_TOKEN|POSTGRES_PASSWORD|DATABASE_URL)=.*$/gm,
  (_, key) => `${key}=${values[key]}`,
);

try {
  writeFileSync(new URL('../.env', import.meta.url), configuration, { flag: 'wx', mode: 0o600 });
  console.log('.env created with random credentials. Your access key is stored in API_TOKEN.');
} catch (error) {
  if (error.code === 'EEXIST') {
    console.error('.env already exists. No files were changed.');
    process.exitCode = 1;
  } else {
    throw error;
  }
}
