import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function collectLines(stream) {
  const lines = [];
  const waiters = [];
  const reader = createInterface({ input: stream });
  reader.on("line", (line) => {
    lines.push(line);
    while (waiters.length && lines.length > waiters[0].index) {
      const waiter = waiters.shift();
      waiter.resolve(lines[waiter.index]);
    }
  });
  return {
    line(index) {
      if (lines.length > index) return Promise.resolve(lines[index]);
      return new Promise((resolve, reject) => waiters.push({ index, resolve, reject }));
    },
    close() {
      reader.close();
      for (const waiter of waiters.splice(0)) waiter.reject(new Error("source closed"));
    },
  };
}

test("homepage is useful and honestly waiting without a trigger endpoint", async ({ page }) => {
  const source = readFileSync("hosts/browser/conduit-site.html", "utf8");
  for (const forbidden of [
    "addEventListener",
    "onclick=",
    "onkeypress=",
    "onkeydown=",
    "https://",
    "http://",
  ]) {
    expect(source).not.toContain(forbidden);
  }
  expect(source).toContain("domHost.completePresentation(effect)");
  expect(source).toContain("if (result.ok) showReceipt(result.receipt)");

  await page.goto("/hosts/browser/conduit-site.html");
  await expect(page).toHaveTitle("Conduit — programs that become real");
  await expect(page.locator("h1")).toHaveText("Meaning, made manifest.");
  await expect(page.locator("#manifestation-title")).toHaveText("Conduit is waiting");
  await expect(page.locator("#proof-status")).toHaveText("waiting for a local WebSocket endpoint");
  await expect(page.locator("#browser-sink output")).toHaveCount(0);
  await expect(page.getByText("Describe meaning")).toBeVisible();
  await expect(page.locator("text=Portable planning")).toBeVisible();
});

test("homepage exposes a bounded error state without fabricating a receipt", async ({ page }) => {
  await page.goto(
    "/hosts/browser/conduit-site.html?ws=ws%3A%2F%2F127.0.0.1%3A1%2Fconduit",
  );
  await expect(page.locator("html")).toHaveAttribute("data-manifestation", "error");
  await expect(page.locator("#manifestation-title")).toHaveText(
    "Conduit could not manifest",
  );
  await expect(page.locator("#manifestation-state")).toHaveText("error");
  await expect(page.locator("#proof-status")).toHaveClass(/error/);
  await expect(page.locator("#browser-sink output")).toHaveCount(0);
});

test("canonical initial state and terminal triggers change the homepage only after presentation completion", async ({ page }) => {
  const source = spawn(
    "cargo",
    ["run", "--quiet", "-p", "conduit-std-host", "--bin", "distributed-toggle-server"],
    { cwd: process.cwd(), stdio: ["pipe", "pipe", "pipe"] },
  );
  const stderr = [];
  source.stderr.setEncoding("utf8");
  source.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = collectLines(source.stdout);
  try {
    const url = await lines.line(0);
    await page.goto(`/hosts/browser/conduit-site.html?ws=${encodeURIComponent(url)}`);
    await expect(page.locator("#browser-sink output")).toHaveCount(1);
    await expect(page.locator("html")).toHaveAttribute("data-manifestation", "true");
    await expect(page.locator("#manifestation-title")).toHaveText("Conduit is live");
    await expect(page.locator("#proof-sequence")).toHaveText("0");

    for (let index = 0; index < 15; index++) {
      await lines.line(index + 1);
      source.stdin.write("\n");
    }
    await expect(page.locator("#proof-status")).toHaveText(
      "completed · receipts=16 · capacity_stable=true",
    );
    const proof = await page.evaluate(() => globalThis.__conduitSiteProof);
    expect(stderr).toEqual([]);
    expect(proof.receiptCount).toBe(16);
    expect(proof.receipts).toHaveLength(16);
    expect(proof.capacityStable).toBe(true);
    expect(proof.closed).toEqual({ ok: true, code: 1000, reason: "conduit-terminal" });
    expect(new Set(proof.receipts.map(({ planId }) => planId)).size).toBe(1);
    for (const field of ["planId", "fragmentId", "activePlayId", "presentationId", "signId"]) {
      await expect(page.locator(`#proof-${field === "activePlayId" ? "play" : field.replace("Id", "")}`))
        .toHaveText(proof.receipts.at(-1)[field]);
    }
  } finally {
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});
