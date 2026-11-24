import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "installed-patchbay.spec.mjs",
  workers: 1,
  retries: 0,
  reporter: [["line"], ["json"]],
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
