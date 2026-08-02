import { defineConfig } from "@playwright/test";

const port = process.env.CONDUIT_PLAYWRIGHT_PORT ?? "4173";
const baseURL = `http://127.0.0.1:${port}`;
const shard = process.env.CONDUIT_PLAYWRIGHT_SHARD ?? "1/1";
const shardCount = Number.parseInt(shard.split("/")[1] ?? "1", 10);

export default defineConfig({
  testDir: "..",
  testMatch: [
    "browser/browser-proof-pages.spec.mjs",
    "browser/conduit-browser-host.spec.mjs",
    "tour/standing-network.spec.mjs",
    "tour/standing-signals.spec.mjs",
    "tour/tour.spec.mjs",
  ],
  // Keep ordinary runs in file order. CI's Firefox shards opt into test-level
  // distribution so the large Tour spec is divided instead of assigned whole
  // to one shard; each shard still executes serially with one worker.
  fullyParallel: shardCount > 1,
  workers: 1,
  retries: 0,
  maxFailures: process.env.CI ? 1 : undefined,
  timeout: 30_000,
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
    command: `cd .. && node browser/static-server.mjs ${port}`,
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
