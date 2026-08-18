import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
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

test("unchanged toggle form runs std kernel to browser WASM kernel over live bounded WebSocket", async ({
  page,
}) => {
  const sourceText = readFileSync("fixtures/forms/remote-toggle.conduit", "utf8");
  for (const forbidden of [
    "websocket",
    "127.0.0.1",
    "browser",
    "dom",
    "stdout",
    "transport",
    "host",
  ]) {
    expect(sourceText.toLowerCase()).not.toContain(forbidden);
  }

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
    process.stdout.write(`distributed toggle URL ${url}\n`);
    await page.goto(
      `/hosts/browser/distributed-toggle.test.html?ws=${encodeURIComponent(url)}`,
    );

    // The canonical current output manifests its exact initial value before any Tick.
    const firstOutput = page.locator("#browser-sink output");
    await expect(firstOutput).toHaveCount(1);
    await expect(firstOutput.first()).toHaveAttribute("data-sequence", "0");
    await expect(firstOutput.first()).toHaveAttribute(
      "data-encoded",
      "[1]",
    );

    // Only after that initial-state proof, send all fifteen admitted triggers.
    for (let i = 0; i < 15; i++) {
      await lines.line(1 + i);
      source.stdin.write("\n");
    }

    await expect(page.locator("#result")).toHaveText(/^ok receipts=16 /);
    const result = await page.evaluate(() => globalThis.__distributedToggleProof);
    process.stdout.write(
      `browser receipts=${result.receiptCount} ` +
      `capacity_stable=${result.capacityStable}\n`,
    );
    await exited;
    const summary = await lines.line(16);
    process.stdout.write(`${summary}\n`);
    expect(stderr).toEqual([]);
    expect(summary).toContain("values=16 pressure_retries=1 retained=0 in_flight=0");
    expect(summary).toContain("source_terminal=completed browser_terminal=completed");
    expect(summary).toContain("capacity_stable=true");
    expect(result.status).toBe(1);
    expect(result.receiptCount).toBe(16);
    expect(result.receipts).toHaveLength(16);
    expect(result.capacityStable).toBe(true);
    expect(result.closed).toEqual({ ok: true, code: 1000, reason: "conduit-terminal" });
    expect(result.presentations.map(({ sequence }) => sequence)).toEqual(
      Array.from({ length: 16 }, (_, index) => index),
    );
    expect(result.receipts.map(({ sequence }) => Number(sequence))).toEqual(
      Array.from({ length: 16 }, (_, index) => index),
    );
    // The toggle sink uses local presentation (kind=2), so receipts contain
    // the local identity chain but not remote identity (sourceHostId, etc.).
    const exactReceiptFields = [
      "sourceDocumentId",
      "checkedFormId",
      "expandedFormId",
      "planId",
      "fragmentId",
      "hostId",
      "bootId",
      "activePlayId",
      "requestNode",
      "requestId",
      "operationId",
      "hostOperationContractId",
      "placementId",
      "presentationId",
      "signId",
    ];
    for (const [index, presentation] of result.presentations.entries()) {
      const receipt = result.receipts[index];
      for (const field of exactReceiptFields) {
        expect(receipt[field], `receipt ${index} ${field}`).toBe(presentation[field]);
      }
      expect(receipt.encoded).toEqual(presentation.encoded);
    }
    expect(result.receipts.every(({ hostId }) =>
      hostId === "s4/toggle-browser-sink")).toBe(true);
    expect(result.receipts.every(({ bootId }) =>
      bootId === "s4/toggle-browser-sink-boot")).toBe(true);
    // Canonical toggle emits initial=true, then alternates after each Tick.
    expect(result.presentations.every(({ encoded }, index) =>
      encoded.length === 1 && encoded[0] === (index % 2 === 0 ? 1 : 0))).toBe(true);
    expect(new Set(result.presentations.map(({ planId }) => planId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ fragmentId }) => fragmentId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ activePlayId }) => activePlayId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ presentationId }) => presentationId)).size).toBe(16);
    expect(new Set(result.presentations.map(({ signId }) => signId)).size).toBe(16);
    expect(new Set(result.presentations.map(({ requestId }) => requestId)).size).toBe(16);
    await expect(page.locator("#result")).toHaveText(
      "ok receipts=16 capacity_stable=true",
    );
    await expect(page.locator("#browser-sink output")).toHaveCount(16);
    await expect(page.locator("#browser-sink output").last()).toHaveAttribute(
      "data-sequence",
      "15",
    );
  } finally {
    if (stderr.length > 0) {
      process.stderr.write(`[toggle server stderr] ${stderr.join("")}\n`);
    }
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});

test("a broken toggle link after four delivered values fails with retained exact receipts", async ({
  page,
}) => {
  const source = spawn(
    "cargo",
    ["run", "--quiet", "-p", "conduit-std-host", "--bin", "distributed-toggle-server"],
    { cwd: process.cwd(), stdio: ["pipe", "pipe", "pipe"] },
  );
  const stderr = [];
  source.stderr.setEncoding("utf8");
  source.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = lineCollector(source.stdout);
  const exitOutcome = processExit(source).then(
    () => ({ ok: true, detail: "zero exit" }),
    (error) => ({ ok: false, detail: error.message }),
  );
  try {
    const url = await lines.line(0);
    expect(url).toMatch(/^ws:\/\/127\.0\.0\.1:\d+\/conduit$/);
    await page.goto("/hosts/browser/distributed-toggle.test.html");

    const browserReady = page.evaluate(async ({ url }) => {
      const { BrowserDomHost } = await import("/hosts/browser/signal-dom-host.mjs");
      const { BrowserWebSocketLine } = await import(
        "/hosts/browser/websocket-line.mjs"
      );
      const {
        instantiateDistributedToggleRuntime,
        runDistributedToggleRuntime,
      } = await import("/hosts/browser/distributed-toggle-runtime.mjs");
      const wasmBytes = await fetch(
        "/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
      ).then((response) => response.arrayBuffer());
      const line = await new BrowserWebSocketLine({
        url,
        maximumMessageBytes: 2048,
        maximumBufferedBytes: 8192,
      }).open();
      const domHost = new BrowserDomHost({
        hostId: "s4/toggle-browser-sink",
        bootId: "s4/toggle-browser-sink-boot",
        root: document.querySelector("#browser-sink"),
        maximumReceiptItems: 16,
        maximumReceiptBytes: 144,
      });
      const runtime = await instantiateDistributedToggleRuntime(wasmBytes);
      const breakingDomHost = {
        completePresentation(effect) {
          const result = domHost.completePresentation(effect);
          if (result.ok && domHost.receipts().length === 4) {
            void line.close(4001, "proof-link-break");
          }
          return result;
        },
      };
      let failure = null;
      try {
        await runDistributedToggleRuntime(runtime, line, breakingDomHost);
      } catch (error) {
        failure = { code: error.code ?? null, message: error.message };
      }
      return {
        failure,
        closed: await line.closed(),
        receipts: domHost.receipts(),
      };
    }, { url });

    // Send Enter presses concurrently; tolerate the server dying mid-flight.
    const enterLoop = (async () => {
      for (let i = 0; i < 16; i++) {
        try {
          await lines.line(1 + i);
        } catch {
          break; // Server exited before all prompts.
        }
        source.stdin.write("\n");
      }
    })();

    const [result] = await Promise.all([browserReady, enterLoop]);
    const exit = await exitOutcome;
    expect(exit.ok).toBe(false);
    expect(exit.detail).toContain("distributed toggle source exit code=1");
    expect(stderr.join("")).toContain("CND-TOG-S4-202 phase=value-in-flight");
    expect(result.failure?.message).toContain("CND-WS-S4-007");
    expect(result.closed).toEqual({ ok: false, code: 1006, reason: "" });
    expect(result.receipts).toHaveLength(4);
    expect(result.receipts.map(({ sequence }) => Number(sequence))).toEqual([0, 1, 2, 3]);
  } finally {
    if (stderr.length > 0) {
      process.stderr.write(`[toggle server stderr] ${stderr.join("")}\n`);
    }
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});
