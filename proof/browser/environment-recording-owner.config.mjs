import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: '.', testMatch: 'environment-recording-owner.spec.mjs',
  fullyParallel: false, workers: 1, retries: 0, timeout: 60_000,
  outputDir: '../../target/environment-recording-source/browser-results', reporter: 'line',
  use: { trace: 'retain-on-failure', viewport: { width: 1280, height: 900 } },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
