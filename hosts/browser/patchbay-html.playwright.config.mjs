import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "patchbay-html.spec.mjs",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 20_000,
  reporter: "line",
  trace: "retain-on-failure",
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "firefox", use: { browserName: "firefox" } },
    { name: "webkit", use: { browserName: "webkit" } },
  ],
});
