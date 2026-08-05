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
        presentations: run.presentations.map((effect) => {
          const { value, ...identity } = effect;
          return {
            ...identity,
            sequence: Number(new DataView(Uint8Array.from(value.encoded).buffer)
              .getBigUint64(0, true)),
            encoded: value.encoded,
          };
        }),
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
    const exactReceiptFields = [
      "sourceDocumentId",
      "checkedFormId",
      "expandedFormId",
      "planId",
      "fragmentId",
      "sourceFragmentId",
      "sourceHostId",
      "sourceBootId",
      "sourceActivePlayId",
      "sourceEndpointId",
      "sinkEndpointId",
      "hostId",
      "bootId",
      "activePlayId",
      "requestNode",
      "requestId",
      "operationId",
      "hostOperationContractId",
      "placementId",
      "presentationId",
      "evidenceId",
      "connectionId",
      "linkBindingId",
      "providerInstanceId",
    ];
    for (const [index, presentation] of result.presentations.entries()) {
      const receipt = result.receipts[index];
      for (const field of exactReceiptFields) {
        expect(receipt[field], `receipt ${index} ${field}`).toBe(presentation[field]);
      }
      expect(receipt.encoded).toEqual(presentation.encoded);
    }
    expect(result.receipts.every(({ sourceHostId }) =>
      sourceHostId === "s4/std-source")).toBe(true);
    expect(result.receipts.every(({ sourceBootId }) =>
      sourceBootId === "s4/std-source-boot")).toBe(true);
    expect(result.receipts.every(({ hostId }) => hostId === "s4/browser-sink")).toBe(true);
    expect(result.receipts.every(({ bootId }) =>
      bootId === "s4/browser-sink-boot")).toBe(true);
    expect(result.receipts.every(({ linkBindingId }) =>
      linkBindingId === "s4/std-browser-link")).toBe(true);
    expect(result.receipts.every(({ providerInstanceId }) =>
      providerInstanceId === "s4/websocket-loopback-instance")).toBe(true);
    expect(new Set(result.presentations.map(({ planId }) => planId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ fragmentId }) => fragmentId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ sourceFragmentId }) => sourceFragmentId)).size)
      .toBe(1);
    expect(new Set(result.presentations.map(({ activePlayId }) => activePlayId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ sourceActivePlayId }) =>
      sourceActivePlayId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ presentationId }) => presentationId)).size).toBe(16);
    expect(new Set(result.presentations.map(({ evidenceId }) => evidenceId)).size).toBe(16);
    expect(new Set(result.presentations.map(({ requestId }) => requestId)).size).toBe(16);
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

test("a broken live link after four delivered values fails with retained exact receipts", async ({
  page,
}) => {
  const source = spawn(
    "cargo",
    ["run", "--quiet", "-p", "conduit-std-host", "--bin", "distributed-signal-server"],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
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
      ).then((response) => response.arrayBuffer());
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
      const breakingDomHost = {
        completePresentation(effect) {
          const result = domHost.completePresentation(effect);
          if (result.ok && domHost.receipts().length === 4) {
            void carrier.close(4001, "proof-link-break");
          }
          return result;
        },
      };
      let failure = null;
      try {
        await runDistributedBrowserRuntime(runtime, carrier, breakingDomHost);
      } catch (error) {
        failure = { code: error.code ?? null, message: error.message };
      }
      return {
        failure,
        closed: await carrier.closed(),
        receipts: domHost.receipts(),
      };
    }, { url });
    const exit = await exitOutcome;
    expect(exit.ok).toBe(false);
    expect(exit.detail).toContain("distributed source exit code=1");
    expect(stderr.join("")).toContain("CND-DST-S4-202 phase=value-in-flight");
    expect(result.failure?.message).toContain("CND-WS-S4-007");
    expect(result.closed).toEqual({ ok: false, code: 1006, reason: "" });
    expect(result.receipts).toHaveLength(4);
    expect(result.receipts.map(({ sequence }) => Number(sequence))).toEqual([0, 1, 2, 3]);
  } finally {
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});
