import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function processExit(child) {
  return new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`distributed toggle source exit code=${code} signal=${signal}`));
    });
  });
}

function lineCollector(stream) {
  const lines = [];
  const waiters = [];
  const reader = createInterface({ input: stream });
  reader.on("line", (line) => {
    lines.push(line);
    while (waiters.length > 0 && lines.length > waiters[0].index) {
      const waiter = waiters.shift();
      waiter.resolve(lines[waiter.index]);
    }
  });
  reader.on("close", () => {
    for (const waiter of waiters.splice(0)) {
      waiter.reject(new Error(`source closed before line ${waiter.index}`));
    }
  });
  return {
    line(index) {
      if (lines.length > index) return Promise.resolve(lines[index]);
      return new Promise((resolve, reject) => waiters.push({ index, resolve, reject }));
    },
    close() {
      reader.close();
    },
  };
}

test("homepage manifestation changes only after a real Conduit browser presentation", async ({ page }) => {
  const source = spawn(
    "cargo",
    ["run", "--quiet", "-p", "conduit-std-host", "--bin", "distributed-toggle-server"],
    { cwd: process.cwd(), stdio: ["pipe", "pipe", "pipe"] },
  );
  const stderr = [];
  source.stderr.setEncoding("utf8");
  source.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = lineCollector(source.stdout);
  const exited = processExit(source);

  try {
    const url = await lines.line(0);
    expect(url).toMatch(/^ws:\/\/127\.0\.0\.1:\d+\/conduit$/);

    await page.goto(`/hosts/browser/conduit-site.html?ws=${encodeURIComponent(url)}`);
    await expect(page.locator("h1")).toContainText("Programs describe meaning");
    await expect(page.locator("html")).toHaveAttribute("data-conduit-level", "waiting");
    await expect(page.locator("#browser-sink output")).toHaveCount(0);

    // The server prints the first admitted activation prompt before stdin is consumed.
    await lines.line(1);
    source.stdin.write("\n");

    // The page may change only after BrowserDomHost accepts the presentation and emits a receipt.
    await expect(page.locator("#browser-sink output")).toHaveCount(1);
    await expect(page.locator("html")).toHaveAttribute("data-conduit-level", "true");
    await expect(page.locator("#signal-state")).toHaveText("LIVE");
    await expect(page.locator("#evidence-sequence")).toHaveText("0");
    await expect(page.locator("#evidence-level")).toHaveText("true");
    await expect(page.locator("#evidence-plan")).not.toHaveText("—");

    // Prove a second exact presentation drives the opposite site state.
    await lines.line(2);
    source.stdin.write("\n");
    await expect(page.locator("#browser-sink output")).toHaveCount(2);
    await expect(page.locator("html")).toHaveAttribute("data-conduit-level", "false");
    await expect(page.locator("#signal-state")).toHaveText("QUIET");
    await expect(page.locator("#evidence-sequence")).toHaveText("1");

    // Complete the planned sixteen-value run so terminal and capacity truth remain intact.
    for (let index = 2; index < 16; index++) {
      await lines.line(1 + index);
      source.stdin.write("\n");
    }

    await exited;
    const proof = await page.evaluate(() => globalThis.__conduitSiteProof);
    expect(stderr).toEqual([]);
    expect(proof.receiptCount).toBe(16);
    expect(proof.receipts).toHaveLength(16);
    expect(proof.capacityStable).toBe(true);
    expect(proof.closed).toEqual({ ok: true, code: 1000, reason: "conduit-terminal" });
    await expect(page.locator("#browser-sink output")).toHaveCount(16);
    await expect(page.locator("#proof-status")).toHaveText("complete · bounded");
    await expect(page.locator("#connection-status")).toHaveText("complete");
  } finally {
    if (stderr.length > 0) {
      process.stderr.write(`[site toggle server stderr] ${stderr.join("")}\n`);
    }
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});
