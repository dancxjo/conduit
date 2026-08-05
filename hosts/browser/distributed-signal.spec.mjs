import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function processExit(child) {
  return new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`distributed source exit code=${code} signal=${signal}`));
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

test("unchanged Signal form runs std kernel to browser WASM kernel over live bounded WebSocket", async ({
  page,
}) => {
  const sourceText = readFileSync("examples/signal-demo.form", "utf8");
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
    ["run", "--quiet", "-p", "conduit-std-host", "--bin", "distributed-signal-server"],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
  );
  const stderr = [];
  source.stderr.setEncoding("utf8");
  source.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = lineCollector(source.stdout);
  const exited = processExit(source);
  try {
    const url = await lines.line(0);
    expect(url).toMatch(/^ws:\/\/127\.0\.0\.1:\d+\/conduit$/);
    process.stdout.write(`distributed Signal URL ${url}\n`);
    await page.goto("/hosts/browser/distributed-signal.test.html");
    const result = await page.evaluate(async ({ url }) => {
      const { BrowserDomHost } = await import("/hosts/browser/signal-dom-host.mjs");
      const { BrowserWebSocketCarrier } = await import(
        "/hosts/browser/websocket-carrier.mjs"
      );
      const {
        instantiateDistributedBrowserRuntime,
        runDistributedBrowserRuntime,
      } = await import("/hosts/browser/distributed-signal-runtime.mjs");
      const wasmBytes = await fetch(
        "/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
      ).then((response) => {
        if (!response.ok) throw new Error("distributed browser WASM artifact missing");
        return response.arrayBuffer();
      });
      const carrier = await new BrowserWebSocketCarrier({
        url,
        maximumMessageBytes: 2048,
        maximumBufferedBytes: 8192,
      }).open();
      const domHost = new BrowserDomHost({
        hostId: "s4/browser-sink",
        bootId: "s4/browser-sink-boot",
        root: document.querySelector("#browser-sink"),
        maximumReceiptItems: 16,
        maximumReceiptBytes: 144,
      });
      const runtime = await instantiateDistributedBrowserRuntime(wasmBytes);
      const run = await runDistributedBrowserRuntime(runtime, carrier, domHost);
      const closed = await carrier.closed();
      const receipts = domHost.receipts();
      document.querySelector("#result").textContent = "ok";
      return {
        ...run,
        presentations: run.presentations.map((effect) => ({
          planId: effect.planId,
          fragmentId: effect.fragmentId,
          activePlayId: effect.activePlayId,
          presentationId: effect.presentationId,
          evidenceId: effect.evidenceId,
          sequence: Number(new DataView(Uint8Array.from(effect.value.encoded).buffer)
            .getBigUint64(0, true)),
          encoded: effect.value.encoded,
        })),
        receipts,
        closed,
      };
    }, { url });
    process.stdout.write(
      `browser receipts=${result.receiptCount} pressure_retries=${result.pressureRetries} ` +
      `retained=${result.retainedValues} in_flight=${result.inFlightItems} ` +
      `capacity_stable=${result.capacityStable}\n`,
    );
    await exited;
    const summary = await lines.line(1);
    process.stdout.write(`${summary}\n`);
    expect(stderr).toEqual([]);
    expect(summary).toContain("values=16 pressure_retries=1 retained=0 in_flight=0");
    expect(summary).toContain("source_terminal=completed browser_terminal=completed");
    expect(summary).toContain("capacity_stable=true");
    expect(result.status).toBe(1);
    expect(result.receiptCount).toBe(16);
    expect(result.receipts).toHaveLength(16);
    expect(result.pressureRetries).toBe(1);
    expect(result.capacityStable).toBe(true);
    expect(result.retainedValues).toBe(0);
    expect(result.inFlightItems).toBe(0);
    expect(result.closed).toEqual({ ok: true, code: 1000, reason: "conduit-terminal" });
    expect(result.presentations.map(({ sequence }) => sequence)).toEqual(
      Array.from({ length: 16 }, (_, index) => index),
    );
    expect(result.receipts.map(({ sequence }) => Number(sequence))).toEqual(
      Array.from({ length: 16 }, (_, index) => index),
    );
    expect(new Set(result.presentations.map(({ planId }) => planId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ fragmentId }) => fragmentId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ activePlayId }) => activePlayId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ presentationId }) => presentationId)).size).toBe(16);
    expect(new Set(result.presentations.map(({ evidenceId }) => evidenceId)).size).toBe(16);
    expect(result.presentations.every(({ encoded }, index) =>
      encoded.length === 9 && encoded[8] === index % 2)).toBe(true);
    await expect(page.locator("#result")).toHaveText("ok");
    await expect(page.locator("#browser-sink output")).toHaveCount(16);
    await expect(page.locator("#browser-sink output").last()).toHaveAttribute(
      "data-sequence",
      "15",
    );
  } finally {
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});
