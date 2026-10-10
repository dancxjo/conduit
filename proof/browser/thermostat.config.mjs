import { defineConfig } from "@playwright/test";
import { fileURLToPath } from "node:url";

export default defineConfig({
  testDir: ".",
  testMatch: "thermostat.spec.mjs",
  workers: 1,
  retries: 0,
  timeout: 90_000,
  use: { baseURL: "http://127.0.0.1:8776", viewport: { width: 1280, height: 1000 }, trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
  webServer: {
    command: "cargo +stable run --locked -p conduit-thermostat-app -- --port 8776",
    cwd: fileURLToPath(new URL("../../", import.meta.url)),
    url: "http://127.0.0.1:8776/face",
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
