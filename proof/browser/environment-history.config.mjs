import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: '.', testMatch: 'environment-history.spec.mjs',
  captureGitInfo: { commit: true, diff: false },
  fullyParallel: false, workers: 1, retries: 0, timeout: 20_000,
  outputDir: 'test-results/environment-history', reporter: 'line',
  use: { trace: 'retain-on-failure', viewport: { width: 1000, height: 1100 } },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
