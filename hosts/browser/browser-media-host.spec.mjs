import { spawn } from "node:child_process";
import { chromium, expect, test } from "@playwright/test";

const entrances = [];
async function startEntrance() {
  const child = spawn("target/debug/conduit-browser-host", ["--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  entrances.push(child);
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`browser Host not ready\n${output}`)), 10_000);
    const inspect = chunk => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", code => reject(new Error(`browser Host exited ${code}\n${output}`)));
  });
  return url;
}

test.afterEach(() => { while (entrances.length) entrances.pop().kill(); });

test("two independent Hosts acquire and consume bounded camera and microphone values", async () => {
  const browser = await chromium.launch({ headless: true, args: ["--use-fake-device-for-media-stream", "--use-fake-ui-for-media-stream"] });
  try {
    const [cameraUrl, microphoneUrl] = await Promise.all([startEntrance(), startEntrance()]);
    const context = await browser.newContext();
    await context.grantPermissions(["camera", "microphone"], { origin: new URL(cameraUrl).origin });
    await context.grantPermissions(["camera", "microphone"], { origin: new URL(microphoneUrl).origin });
    const camera = await context.newPage();
    const microphone = await context.newPage();
    await Promise.all([camera.goto(cameraUrl), microphone.goto(microphoneUrl)]);
    await Promise.all([camera.getByRole("button", { name: "Acquire camera" }).click(), microphone.getByRole("button", { name: "Acquire microphone" }).click()]);
    await Promise.all([expect(camera.locator("#media-status")).toContainText("MediaClosed"), expect(microphone.locator("#media-status")).toContainText("MediaClosed")]);
    const read = page => page.evaluate(() => ({ identity: { hostId: __conduitBrowserHost.hostId, bootId: __conduitBrowserHost.bootId }, evidence: __conduitBrowserHost.media.evidence() }));
    const [cameraProof, microphoneProof] = await Promise.all([read(camera), read(microphone)]);
    expect(cameraProof.identity.hostId).not.toBe(microphoneProof.identity.hostId);
    expect(cameraProof.identity.bootId).not.toBe(microphoneProof.identity.bootId);
    expect(cameraProof.evidence.observed_values).toBe(1);
    expect(microphoneProof.evidence.observed_values).toBe(1);
    expect(cameraProof.evidence.last_value_checksum).not.toBe(0);
    expect(microphoneProof.evidence.last_value_checksum).not.toBe(0);
  } finally { await browser.close(); }
});

for (const [name, domName, terminal] of [
  ["denial", "NotAllowedError", "AcquisitionDenied"],
  ["dismissal", "AbortError", "AcquisitionDismissed"],
  ["unsupported constraints", "OverconstrainedError", "UnsupportedConstraints"],
]) {
  test(`deterministic adapter preserves ${name}`, async ({ page }) => {
    const url = await startEntrance();
    await page.addInitScript(({ domName }) => {
      Object.defineProperty(navigator, "mediaDevices", { value: { getUserMedia: async () => { throw new DOMException("fixture", domName); } } });
    }, { domName });
    await page.goto(url);
    await page.getByRole("button", { name: "Acquire camera" }).click();
    await expect(page.locator("#media-status")).toContainText(terminal);
    expect(await page.evaluate(() => __conduitBrowserHost.media.evidence().observed_values)).toBe(0);
  });
}
