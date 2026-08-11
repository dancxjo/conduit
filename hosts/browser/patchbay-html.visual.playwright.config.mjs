import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "patchbay-html.visual.spec.mjs",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 20_000,
  reporter: "line",
  outputDir: "../../target/playwright/patchbay-visual",
  updateSnapshots: "none",
  projects: [
    {
      name: "chromium",
      use: {
        browserName: "chromium",
        viewport: { width: 1440, height: 1000 },
        deviceScaleFactor: 1,
        locale: "en-US",
        timezoneId: "UTC",
        colorScheme: "dark",
        reducedMotion: "reduce",
        trace: "retain-on-failure",
      },
    },
  ],
});
