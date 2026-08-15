import { defineConfig } from "@playwright/test";

const port = process.env.CONDUIT_BROWSER_HOST_PORT ?? "4173";

export default defineConfig({
  testDir: ".",
  testMatch: [
    "signal-dom-host.spec.mjs",
    "websocket-line.spec.mjs",
    "distributed-signal.spec.mjs",
    "distributed-toggle.spec.mjs",
    "conduit-site.spec.mjs",
    "webchat.spec.mjs",
    "pool-webchat.spec.mjs",
    "r1-three-peer-input.spec.mjs",
    "triple-signal.spec.mjs",
    "presentation-nucleus.spec.mjs",
    "browser-presence.spec.mjs",
    "webrtc-datachannel-line.spec.mjs",
  ],
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
    url: `http://127.0.0.1:${port}/hosts/browser/signal-dom-host.test.html`,
    reuseExistingServer: !process.env.CI,
    timeout: 10_000,
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
