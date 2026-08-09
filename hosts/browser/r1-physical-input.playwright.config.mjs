import { defineConfig } from "@playwright/test";

const port = process.env.CONDUIT_R1_INPUT_HTTP_PORT ?? "4179";

export default defineConfig({
  testDir: ".",
  testMatch: ["r1-physical-input.spec.mjs"],
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 20_000,
  reporter: "line",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
  },
  webServer: {
    command: `cd ../.. && node hosts/browser/static-server.mjs ${port}`,
    url: `http://127.0.0.1:${port}/hosts/browser/r1-three-peer-input.test.html`,
    reuseExistingServer: false,
    timeout: 10_000,
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
