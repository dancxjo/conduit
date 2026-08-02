import { defineConfig } from "@playwright/test";

const port = process.env.CONDUIT_PLAYWRIGHT_PORT ?? "4173";
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "..",
  testMatch: [
    "browser/conduit-browser-host.spec.mjs",
    "tour/standing-network.spec.mjs",
    "tour/standing-signals.spec.mjs",
    "tour/tour.spec.mjs",
  ],
  fullyParallel: false,
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
