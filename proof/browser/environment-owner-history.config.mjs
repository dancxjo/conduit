import base, { browserProofPort } from './playwright.config.mjs';
const port = browserProofPort(process.env.CONDUIT_BROWSER_HOST_PORT ?? '18457');
export default {
  ...base, testMatch: 'environment-owner-history.spec.mjs',
  workers: 1, retries: 0, fullyParallel: false,
  outputDir: 'test-results/environment-owner-history',
  use: { ...base.use, baseURL: `http://127.0.0.1:${port}` },
  webServer: {
    ...base.webServer, reuseExistingServer: false,
    command: `cd ../.. && node proof/browser/static-server.mjs ${port}`,
    url: `http://127.0.0.1:${port}/proof/browser/environment-owner-history.test.html`,
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
};
