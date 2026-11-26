import { defineConfig } from '@playwright/test';
import path from 'node:path';
export default defineConfig({
  testDir: '.', testMatch: 'environment-recording-owner.spec.mjs',
  fullyParallel: false, workers: 1, retries: 0, timeout: 60_000,
  outputDir: process.env.CONDUIT_RECORDING_BROWSER_OUTPUT
    ? path.join(path.dirname(process.env.CONDUIT_RECORDING_BROWSER_OUTPUT), 'playwright-results')
    : '../../target/environment-recording-source/browser-results', reporter: 'line',
  use: { trace: 'retain-on-failure', viewport: { width: 1280, height: 900 } },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
