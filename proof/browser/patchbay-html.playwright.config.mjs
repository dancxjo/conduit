import { defineConfig } from "@playwright/test";

const captureRestartProbe=process.env.CONDUIT_CAPTURE_RESTART_PROBE==="1";

export default defineConfig({
  testDir: ".",
  testMatch: captureRestartProbe
    ? ["capture-restart-probe.spec.mjs"]
    : ["patchbay-html.spec.mjs", "patchbay-body-workbench.spec.mjs", "patchbay-body-execution.spec.mjs", "patchbay-front-door.spec.mjs", "patchbay-recursive-form.spec.mjs", "patchbay-authoring.spec.mjs", "patchbay-cord-routing.spec.mjs", "patchbay-follow.spec.mjs", "patchbay-navigation-conformance.spec.mjs", "patchbay-depth.spec.mjs", "patchbay-panel-furniture.spec.mjs", "patchbay-text-lab-split.spec.mjs", "patchbay-llm.spec.mjs"],
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 20_000,
  reporter: "line",
  trace: "retain-on-failure",
  projects: [
    {
      name: "chromium",
      use: {
        browserName: "chromium",
        viewport: { width: 1366, height: 768 },
        deviceScaleFactor: 1,
        locale: "en-US",
        timezoneId: "UTC",
        colorScheme: "dark",
        reducedMotion: "reduce",
      },
    },
  ],
});
