import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";

async function startPresenceProbe(extraArguments = []) {
  const process = spawn("target/debug/browser-admission-probe", ["--presence", ...extraArguments], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`presence probe was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/^(ws:\/\/[^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    process.stdout.on("data", inspect);
    process.stderr.on("data", inspect);
    process.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`presence probe exited (${code})\n${output}`));
    });
  });
  return { process, url, output: () => output };
}

test("admitted browser renews exact current presence and close makes it unavailable", async ({ page }) => {
  const probe = await startPresenceProbe();
  await page.goto(`/hosts/browser/browser-presence.test.html?body=${encodeURIComponent(probe.url)}`);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence?.presenceState())).toBe("available");
  await expect.poll(probe.output).toContain("renewed sequence=2");
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.freshnessProfile())).toMatchObject({
    scheduling: "best-effort-browser-event-loop",
    availabilityAuthority: "server-session-or-lease",
    backgroundRealtimeGuarantee: false,
    maximumReconnectAttempts: 1,
    sequence: 2,
    renewAfterMillis: 500,
  });
  expect(["visible", "hidden"]).toContain(
    await page.evaluate(() => globalThis.__browserPresence.pageLifecycle()),
  );
  await page.evaluate(() => globalThis.__browserPresence.close());
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.state())).toBe("offline");
  await expect.poll(probe.output).toContain("unavailable reason=session-lost sequence=2");
  await expect.poll(() => probe.process.exitCode).toBe(0);
});

test("admitted browser that stops renewing becomes unavailable only at lease expiry", async ({ page }) => {
  const probe = await startPresenceProbe();
  await page.goto(`/hosts/browser/browser-presence.test.html?renew=false&reconnect=false&body=${encodeURIComponent(probe.url)}`);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence?.presenceState())).toBe("available");
  await expect.poll(probe.output).toContain("unavailable reason=expired sequence=1");
  await expect.poll(() => probe.process.exitCode).toBe(0);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.state())).toBe("offline");
});

test("same running browser returns after session loss with exact Host and Boot", async ({ page }) => {
  const probe = await startPresenceProbe(["--reconnect"]);
  await page.goto(`/hosts/browser/browser-presence.test.html?body=${encodeURIComponent(probe.url)}`);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence?.presenceState())).toBe("available");
  const identity = await page.evaluate(() => ({
    hostId: globalThis.__browserPresence.hostId,
    bootId: globalThis.__browserPresence.bootId,
  }));
  await expect.poll(probe.output).toContain("unavailable reason=session-lost-for-return sequence=2");
  await expect.poll(probe.output).toContain("returned part=");
  await expect.poll(probe.output).toContain("returned-renewed sequence=4");
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.state())).toBe("admitted");
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.presenceState())).toBe("available");
  expect(await page.evaluate(() => ({
    hostId: globalThis.__browserPresence.hostId,
    bootId: globalThis.__browserPresence.bootId,
  }))).toEqual(identity);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.freshnessProfile())).toMatchObject({
    sequence: 4,
    maximumReconnectAttempts: 1,
    backgroundRealtimeGuarantee: false,
  });
  await page.evaluate(() => globalThis.__browserPresence.close());
  await expect.poll(probe.output).toContain("returned-unavailable reason=session-lost");
  await expect.poll(() => probe.process.exitCode).toBe(0);
});
