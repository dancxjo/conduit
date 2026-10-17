import { defineConfig } from '../browser/node_modules/@playwright/test/index.mjs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const directory = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(directory, '../..');
const server = process.env.CONDUIT_BROWSER_PROOF_STATIC_SERVER || path.join(root, 'proof/browser/static-server.mjs');
const quote = value => "'" + value.replaceAll("'", "'\"'\"'") + "'";
export default defineConfig({
  testDir: directory, testMatch: 'browser-overhead.spec.mjs', workers: 1, retries: 0,
  captureGitInfo: { commit: true, diff: false }, reporter: 'list',
  outputDir: path.join(root, 'target/browser-overhead'),
  use: { baseURL: 'http://127.0.0.1:54876', headless: true },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
  webServer: { command: [process.execPath, server, '54876', root].map(quote).join(' '),
    cwd: root, url: 'http://127.0.0.1:54876/proof/browser/native-value-types.test.html',
    reuseExistingServer: false, timeout: 10000 },
});
