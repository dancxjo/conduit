import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

test("real browser monotonic timer ignores wall-clock steps", async ({ page }) => {
  await page.goto("/proof/browser/signal-dom-host.test.html");
  const result = await page.evaluate(async () => {
    const { createBrowserMonotonicTimer } = await import(
      "/targets/browser/host/assets/browser-monotonic-timer.mjs"
    );
    const timer = createBrowserMonotonicTimer(window, "host/browser", "boot/browser");
    const originalDateNow = Date.now;
    try {
      const before = timer.nowMicros();
      Date.now = () => originalDateNow() + 60_000;
      await timer.wait(20, new AbortController().signal, { pending: null, cancel: null });
      const afterForwardStep = timer.nowMicros();
      Date.now = () => originalDateNow() - 60_000;
      await timer.wait(20, new AbortController().signal, { pending: null, cancel: null });
      const afterBackwardStep = timer.nowMicros();
      timer.close();
      let closedRefusal = false;
      try {
        timer.nowMicros();
      } catch {
        closedRefusal = true;
      }
      return { before, afterForwardStep, afterBackwardStep, closedRefusal };
    } finally {
      Date.now = originalDateNow;
    }
  });
  expect(result.afterForwardStep - result.before).toBeGreaterThanOrEqual(20_000);
  expect(result.afterBackwardStep - result.afterForwardStep).toBeGreaterThanOrEqual(20_000);
  expect(result.closedRefusal).toBe(true);
});

test("Chromium and native std process reconcile their distinct monotonic bases", async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto("/proof/browser/signal-dom-host.test.html");
  const browserBasis = await page.evaluate(async () => {
    const { createBrowserMonotonicTimer } = await import(
      "/targets/browser/host/assets/browser-monotonic-timer.mjs"
    );
    globalThis.__bodyClock = createBrowserMonotonicTimer(window, "host/browser", "boot/browser");
    return globalThis.__bodyClock.basisId;
  });
  const native = spawn("cargo", [
    "+stable", "run", "-q", "-p", "conduit-core", "--example", "body_time_browser_peer", "--locked",
  ], {
    cwd: process.cwd(),
    env: {
      ...process.env,
      CARGO_INCREMENTAL: "0",
      CARGO_PROFILE_DEV_DEBUG: "0",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let nativeErrors = "";
  native.stderr.on("data", (chunk) => { nativeErrors += chunk.toString(); });
  const lines = createInterface({ input: native.stdout })[Symbol.asyncIterator]();
  const readLine = async () => {
    const { value, done } = await lines.next();
    if (done) throw new Error(`native clock peer ended early: ${nativeErrors}`);
    return value.trim().split(" ");
  };
  try {
    expect(await readLine()).toEqual(["READY"]);
    const estimates = [];
    for (let round = 0; round < 2; round += 1) {
      const send = await page.evaluate(() => globalThis.__bodyClock.nowMicros());
      native.stdin.write(`PROBE ${send} ${browserBasis}\n`);
      const [sampleTag, nativeReceive, nativeSend] = await readLine();
      expect(sampleTag).toBe("SAMPLES");
      expect(Number(nativeSend)).toBeGreaterThanOrEqual(Number(nativeReceive));
      const receive = await page.evaluate(() => globalThis.__bodyClock.nowMicros());
      native.stdin.write(`COMPLETE ${receive}\n`);
      const [estimateTag, generation, center, earliest, latest, age, host, boot, basis] = await readLine();
      expect(estimateTag).toBe("ESTIMATE");
      expect(Number(generation)).toBe(round + 2);
      expect(Number(earliest)).toBeLessThanOrEqual(Number(center));
      expect(Number(center)).toBeLessThanOrEqual(Number(latest));
      expect(Number(latest) - Number(earliest)).toBeLessThan(500_000);
      expect(Number(age)).toBe(0);
      expect([host, boot, basis]).toEqual(["host/browser", "boot/browser", browserBasis]);
      estimates.push(Number(center));
      if (round === 0) await page.waitForTimeout(100);
    }
    expect(estimates[1]).toBeGreaterThan(estimates[0]);
    native.stdin.end();
    await expect.poll(() => native.exitCode).toBe(0);
  } finally {
    await page.evaluate(() => globalThis.__bodyClock?.close());
    if (native.exitCode === null) native.kill();
  }
});
