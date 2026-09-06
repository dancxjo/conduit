import { defineConfig } from "@playwright/test";

export function browserProofPort(value = "4173") {
  if (!/^[0-9]{4,5}$/.test(value)) throw new Error("CONDUIT_BROWSER_HOST_PORT must be a decimal loopback port");
  const port = Number(value);
  if (!Number.isSafeInteger(port) || port < 1024 || port > 65_535) {
    throw new Error("CONDUIT_BROWSER_HOST_PORT must be between 1024 and 65535");
  }
  return String(port);
}

export function browserProofShard(value = "default") {
  if (!/^[a-z0-9][a-z0-9_-]{0,47}$/.test(value)) {
    throw new Error("CONDUIT_BROWSER_PROOF_SHARD must be a bounded local identity");
  }
  return value;
}

const port = browserProofPort(process.env.CONDUIT_BROWSER_HOST_PORT);
const shard = browserProofShard(process.env.CONDUIT_BROWSER_PROOF_SHARD);

export default defineConfig({
  testDir: ".",
  testMatch: [
    "signal-dom-host.spec.mjs",
    "websocket-line.spec.mjs",
    "distributed-signal.spec.mjs",
    "text-lab-live.spec.mjs",
    "distributed-toggle.spec.mjs",
    "conduit-site.spec.mjs",
    "webchat.spec.mjs",
    "pool-webchat.spec.mjs",
    "r1-three-peer-input.spec.mjs",
    "triple-signal.spec.mjs",
    "presentation-nucleus.spec.mjs",
    "fourth-product-conformance.spec.mjs",
    "browser-host-operations.spec.mjs",
    "browser-pointer.spec.mjs",
    "browser-human-input.spec.mjs",
    "browser-host-entrance.spec.mjs",
    "tour.spec.mjs",
    "browser-application-package.spec.mjs",
    "browser-bundle-build.spec.mjs",
    "browser-boot-profile.spec.mjs",
    "browser-form-runner.spec.mjs",
    "quantity-controller.spec.mjs",
    "reviewed-form-conformance.spec.mjs",
    "pages-front-door.spec.mjs",
    "creche-avr.spec.mjs",
    "creche-orange-pi.spec.mjs",
    "creche-raspberry-pi.spec.mjs",
    "creche-conduitos.spec.mjs",
    "creche-native-zip.spec.mjs",
    "creche-native-disk.spec.mjs",
    "creche-browser-configuration.spec.mjs",
    "creche-workload.spec.mjs",
    "creche-naming.spec.mjs",
    "browser-media-host.spec.mjs",
    "browser-device-base.spec.mjs",
    "browser-usb-device-base.spec.mjs",
    "rp2040-browser-deployment.spec.mjs",
    "esp32-browser-deployment.spec.mjs",
    "browser-body-camera-realization.spec.mjs",
    "browser-presence.spec.mjs",
    "browser-webrtc-body.spec.mjs",
    "firefly-choir.spec.mjs",
    "human-interaction-presenter.spec.mjs",
    "human-interaction-convergence.spec.mjs",
    "webrtc-datachannel-line.spec.mjs",
  ],
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 20_000,
  outputDir: `test-results/${shard}`,
  reporter: "line",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
  },
  webServer: {
    command: `cd ../.. && node proof/browser/static-server.mjs ${port}`,
    url: `http://127.0.0.1:${port}/proof/browser/signal-dom-host.test.html`,
    reuseExistingServer: !process.env.CI,
    timeout: 10_000,
  },
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    {
      name: "firefox",
      testMatch: ["browser-webrtc-body.spec.mjs"],
      use: { browserName: "firefox" },
    },
  ],
});
