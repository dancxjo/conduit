import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";

async function startPresenceProbe() {
  const process = spawn("target/debug/browser-admission-probe", ["--presence"], {
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
  await page.evaluate(() => globalThis.__browserPresence.close());
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.state())).toBe("offline");
  await expect.poll(probe.output).toContain("unavailable reason=session-lost sequence=2");
  await expect.poll(() => probe.process.exitCode).toBe(0);
});

test("admitted browser that stops renewing becomes unavailable only at lease expiry", async ({ page }) => {
  const probe = await startPresenceProbe();
  await page.goto(`/hosts/browser/browser-presence.test.html?renew=false&body=${encodeURIComponent(probe.url)}`);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence?.presenceState())).toBe("available");
  await expect.poll(probe.output).toContain("unavailable reason=expired sequence=1");
  await expect.poll(() => probe.process.exitCode).toBe(0);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.state())).toBe("offline");
});
