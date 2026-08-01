import { defineConfig } from "@playwright/test";

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
  timeout: 30_000,
  reporter: "line",
  use: {
    baseURL: "http://127.0.0.1:4173",
    serviceWorkers: "allow",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "cd .. && node browser/static-server.mjs",
    url: "http://127.0.0.1:4173/browser/conduit-browser-host.test.html",
    reuseExistingServer: !process.env.CI,
    timeout: 10_000,
  },
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "firefox", use: { browserName: "firefox" } },
    { name: "webkit", use: { browserName: "webkit" } },
  ],
});
