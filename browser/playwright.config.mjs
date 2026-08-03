import { defineConfig } from "@playwright/test";

const port = process.env.CONDUIT_PLAYWRIGHT_PORT ?? "4173";
const baseURL = `http://127.0.0.1:${port}`;
const shard = process.env.CONDUIT_PLAYWRIGHT_SHARD ?? "1/1";
const shardCount = Number.parseInt(shard.split("/")[1] ?? "1", 10);
const testTimeoutMs = Number.parseInt(
  process.env.CONDUIT_PLAYWRIGHT_TEST_TIMEOUT_MS ?? "30000",
  10,
);
const testHostToken = process.env.CONDUIT_TEST_HOST_TOKEN ??
  "conduit-playwright-task-action-host";

if (!Number.isSafeInteger(testTimeoutMs) || testTimeoutMs <= 0) {
  throw new Error("CONDUIT_PLAYWRIGHT_TEST_TIMEOUT_MS must be a positive integer");
}

export default defineConfig({
  testDir: "..",
  testMatch: [
    "browser/browser-proof-pages.spec.mjs",
    "browser/conduit-browser-host.spec.mjs",
    "tour/standing-network.spec.mjs",
    "tour/standing-signals.spec.mjs",
    "tour/task-action-policy-proof.spec.mjs",
    "tour/tour.spec.mjs",
    "tour/workbench-responsive.spec.mjs",
    "tour/workbench.spec.mjs",
  ],
  // Keep ordinary runs in file order. CI's Firefox shards opt into test-level
  // distribution so the large Tour spec is divided instead of assigned whole
  // to one shard; each shard still executes serially with one worker.
  fullyParallel: shardCount > 1,
  workers: 1,
  retries: 0,
  maxFailures: process.env.CI ? 1 : undefined,
  timeout: testTimeoutMs,
  reporter: process.env.CI
    ? [["line"], ["blob", {
      outputDir: process.env.PLAYWRIGHT_BLOB_OUTPUT_DIR ?? "blob-report",
    }]]
    : "line",
  use: {
    baseURL,
    serviceWorkers: "allow",
    trace: "retain-on-failure",
  },
  webServer: {
    command: `cd .. && CONDUIT_TOUR_SITE=target/tour-site CONDUIT_TEST_HOST_TOKEN=${testHostToken} node browser/static-server.mjs ${port}`,
    url: `${baseURL}/browser/conduit-browser-host.test.html`,
    reuseExistingServer: !process.env.CI,
    timeout: 10_000,
  },
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "firefox", use: { browserName: "firefox" } },
    { name: "webkit", use: { browserName: "webkit" } },
  ],
});
