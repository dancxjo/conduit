import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: ["patchbay-llm-embodiment.spec.mjs"],
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 20_000,
  reporter: "line",
  projects: [{
    name: "chromium",
    use: {
      browserName: "chromium",
      viewport: { width: 1366, height: 768 },
      locale: "en-US",
      timezoneId: "UTC",
      reducedMotion: "reduce",
    },
  }],
});
