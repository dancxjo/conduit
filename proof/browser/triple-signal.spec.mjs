import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

test.skip(process.env.CONDUIT_THREE_HOST !== "1", "requires the attached accepted Pico W");

function collectLines(stream) {
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
      waiter.reject(new Error(`three-host source closed before line ${waiter.index}`));
    }
  });
  return {
    line(index) {
      if (lines.length > index) return Promise.resolve(lines[index]);
      return new Promise((resolve, reject) => waiters.push({ index, resolve, reject }));
    },
    close() { reader.close(); },
  };
}

function exitOutcome(child) {
  return new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`three-host source exit code=${code} signal=${signal}`));
    });
  });
}

test("one unchanged form produces matching stdout, DOM, and physical Pico LED receipts", async ({
  page,
}) => {
  test.skip(process.env.CONDUIT_THREE_HOST_FAILURE === "1", "running the failure proof only");
  test.setTimeout(60_000);
  const form = readFileSync("fixtures/forms/triple-signal.conduit", "utf8").toLowerCase();
  for (const forbidden of [
    "stdout", "dom", "gpio", "transport", "usb", "websocket", "browser", "pico",
    "firmware", "host", "address", "socket",
  ]) {
    expect(form).not.toContain(forbidden);
  }

  const source = spawn(
    "cargo",
    ["run", "--quiet", "-p", "conduit-std-host", "--bin", "triple-signal-server"],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
  );
  const stderr = [];
  source.stderr.setEncoding("utf8");
  source.stderr.on("data", (chunk) => {
    stderr.push(chunk);
    process.stderr.write(chunk);
  });
  const lines = collectLines(source.stdout);
  const exited = exitOutcome(source);
  try {
    const url = await lines.line(0);
    expect(url).toMatch(/^ws:\/\/127\.0\.0\.1:\d+\/conduit$/);
    await page.goto("/proof/browser/distributed-signal.test.html");
    const result = await page.evaluate(async ({ url }) => {
      const { BrowserDomHost } = await import("/proof/browser/signal-dom-host.mjs");
      const { BrowserWebSocketLine } = await import(
        "/hosts/browser-host/assets/websocket-line.mjs"
      );
      const {
        instantiateDistributedBrowserRuntime,
        runDistributedBrowserRuntime,
      } = await import("/proof/browser/distributed-signal-runtime.mjs");
      const wasmBytes = await fetch(
        "/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
      ).then((response) => {
        if (!response.ok) throw new Error("three-host browser WASM artifact missing");
        return response.arrayBuffer();
      });
      const line = await new BrowserWebSocketLine({
        url,
        maximumMessageBytes: 2048,
        maximumBufferedBytes: 8192,
      }).open();
      const domHost = new BrowserDomHost({
        hostId: "s4/triple-browser",
        bootId: "s4/triple-browser-boot",
        root: document.querySelector("#browser-sink"),
        maximumReceiptItems: 16,
        maximumReceiptBytes: 144,
      });
      const runtime = await instantiateDistributedBrowserRuntime(wasmBytes, { triple: true });
      const run = await runDistributedBrowserRuntime(runtime, line, domHost);
      return {
        ...run,
        receipts: domHost.receipts(),
        closed: await line.closed(),
      };
    }, { url });
    await exited;

    const stdoutReceipts = [];
    for (let index = 1; index <= 16; index += 1) {
      stdoutReceipts.push(JSON.parse(await lines.line(index)));
    }
    const summary = await lines.line(17);
    expect(stderr).toEqual([]);
    expect(summary).toContain("values=16 stdout_receipts=16 browser_receipts=16 pico_receipts=16");
    expect(summary).toContain("terminal=completed");
    expect(summary).toContain("browser_link=s4/triple-std-browser-link");
    expect(summary).toContain("pico_link=s4/triple-std-pico-link");
    expect(summary).not.toContain("firmware_build=missing");

    const expectedSequences = Array.from({ length: 16 }, (_, index) => index);
    const expectedLevels = expectedSequences.map((sequence) => sequence % 2 === 1);
    expect(stdoutReceipts.map(({ sequence }) => sequence)).toEqual(expectedSequences);
    expect(stdoutReceipts.map(({ level }) => level)).toEqual(expectedLevels);
    expect(result.receipts.map(({ sequence }) => Number(sequence))).toEqual(expectedSequences);
    expect(result.receipts.map(({ level }) => level)).toEqual(expectedLevels);
    expect(result.presentations.map(({ value }) => value.encoded[8] === 1)).toEqual(expectedLevels);
    expect(result.status).toBe(1);
    expect(result.receiptCount).toBe(16);
    expect(result.pressureRetries).toBe(1);
    expect(result.capacityStable).toBe(true);
    expect(result.retainedValues).toBe(0);
    expect(result.inFlightItems).toBe(0);
    expect(result.closed).toEqual({ ok: true, code: 1000, reason: "conduit-terminal" });
    expect(result.receipts.every(({ planId }) => planId === stdoutReceipts[0].plan_id)).toBe(true);
    expect(result.receipts.every(({ hostId }) => hostId === "s4/triple-browser")).toBe(true);
    expect(result.receipts.every(({ linkBindingId }) =>
      linkBindingId === "s4/triple-std-browser-link")).toBe(true);
    expect(new Set(stdoutReceipts.map(({ presentation_id }) => presentation_id)).size).toBe(16);
    expect(new Set(stdoutReceipts.map(({ sign_id }) => sign_id)).size).toBe(16);
    await expect(page.locator("#browser-sink output")).toHaveCount(16);
    await expect(page.locator("#browser-sink output").last()).toHaveAttribute(
      "data-sequence", "15",
    );
  } finally {
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});

test("a broken browser link fails the one kernel and reaches a physical Pico terminal", async ({
  page,
}) => {
  test.skip(process.env.CONDUIT_THREE_HOST_FAILURE !== "1", "opt-in destructive failure proof");
  test.setTimeout(60_000);
  const source = spawn(
    "cargo",
    ["run", "--quiet", "-p", "conduit-std-host", "--bin", "triple-signal-server"],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
  );
  const stderr = [];
  source.stderr.setEncoding("utf8");
  source.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = collectLines(source.stdout);
  const exited = exitOutcome(source).then(
    () => ({ ok: true, detail: "zero exit" }),
    (error) => ({ ok: false, detail: error.message }),
  );
  try {
    const url = await lines.line(0);
    await page.goto("/proof/browser/distributed-signal.test.html");
    const result = await page.evaluate(async ({ url }) => {
      const { BrowserDomHost } = await import("/proof/browser/signal-dom-host.mjs");
      const { BrowserWebSocketLine } = await import(
        "/hosts/browser-host/assets/websocket-line.mjs"
      );
      const {
        instantiateDistributedBrowserRuntime,
        runDistributedBrowserRuntime,
      } = await import("/proof/browser/distributed-signal-runtime.mjs");
      const wasmBytes = await fetch(
        "/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
      ).then((response) => response.arrayBuffer());
      const line = await new BrowserWebSocketLine({
        url,
        maximumMessageBytes: 2048,
        maximumBufferedBytes: 8192,
      }).open();
      const domHost = new BrowserDomHost({
        hostId: "s4/triple-browser",
        bootId: "s4/triple-browser-boot",
        root: document.querySelector("#browser-sink"),
        maximumReceiptItems: 16,
        maximumReceiptBytes: 144,
      });
      const runtime = await instantiateDistributedBrowserRuntime(wasmBytes, { triple: true });
      const breakingDomHost = {
        completePresentation(effect) {
          const completion = domHost.completePresentation(effect);
          if (completion.ok && domHost.receipts().length === 4) {
            void line.close(4001, "three-host-proof-link-break");
          }
          return completion;
        },
      };
      let failure;
      try {
        await runDistributedBrowserRuntime(runtime, line, breakingDomHost);
      } catch (error) {
        failure = error.message;
      }
      return { failure, receipts: domHost.receipts(), closed: await line.closed() };
    }, { url });
    const exit = await exited;
    const stdoutReceipts = [];
    for (let index = 1; index <= 4; index += 1) {
      stdoutReceipts.push(JSON.parse(await lines.line(index)));
    }
    const summary = await lines.line(5);
    expect(exit.ok).toBe(false);
    expect(exit.detail).toContain("three-host source exit code=1");
    expect(stderr.join("")).toContain("failure propagated to Pico terminal");
    expect(summary).toContain("values=4 terminal=failed failure_code=350");
    expect(summary).toContain("pico_link=s4/triple-std-pico-link");
    expect(result.failure).toContain("CND-WS-S4-007");
    expect(result.receipts).toHaveLength(4);
    expect(result.receipts.map(({ sequence }) => Number(sequence))).toEqual([0, 1, 2, 3]);
    expect(stdoutReceipts.map(({ sequence }) => sequence)).toEqual([0, 1, 2, 3]);
    expect(result.closed.ok).toBe(false);
  } finally {
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});
