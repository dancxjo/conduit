import { spawn } from "node:child_process";
import { readFileSync, rmSync } from "node:fs";
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
  const matchWaiters = [];
  const reader = createInterface({ input: stream });
  reader.on("line", (line) => {
    lines.push(line);
    while (waiters.length > 0 && lines.length > waiters[0].index) {
      const waiter = waiters.shift();
      waiter.resolve(lines[waiter.index]);
    }
    for (let index = matchWaiters.length - 1; index >= 0; index -= 1) {
      const waiter = matchWaiters[index];
      if (waiter.pattern.test(line)) {
        matchWaiters.splice(index, 1);
        waiter.resolve(line);
      }
    }
  });
  reader.on("close", () => {
    for (const waiter of waiters.splice(0)) {
      waiter.reject(new Error(`source closed before line ${waiter.index}`));
    }
    for (const waiter of matchWaiters.splice(0)) {
      waiter.reject(new Error(`source closed before matching ${waiter.pattern}`));
    }
  });
  return {
    line(index) {
      if (lines.length > index) return Promise.resolve(lines[index]);
      return new Promise((resolve, reject) => waiters.push({ index, resolve, reject }));
    },
    matching(pattern) {
      const found = lines.find((line) => pattern.test(line));
      if (found !== undefined) return Promise.resolve(found);
      return new Promise((resolve, reject) => {
        matchWaiters.push({ pattern, resolve, reject });
      });
    },
    close() {
      reader.close();
    },
  };
}

test("unchanged Signal form runs std kernel to browser WASM kernel over live bounded WebSocket", async ({
  page,
}) => {
  const reportPath = `/tmp/conduit-browser-product-${process.pid}.json`;
  const sourceText = readFileSync("fixtures/forms/signal-demo.conduit", "utf8");
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
    "target/debug/conduit",
    [
      "run",
      "fixtures/forms/signal-demo.conduit",
      "--execution-fixture",
      "std-browser-line",
      "--report",
      reportPath,
    ],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
  );
  const stderr = [];
  source.stderr.setEncoding("utf8");
  source.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = lineCollector(source.stdout);
  const exited = processExit(source);
  try {
    const bootstrap = await lines.line(0);
    const bootstrapMatch = bootstrap.match(
      /^browser_product url=(ws:\/\/127\.0\.0\.1:\d+\/conduit) source_host=(\S+) source_boot=(\S+) browser_host=(\S+) browser_boot=(\S+) plan=(\S+)$/,
    );
    expect(bootstrapMatch).not.toBeNull();
    const [
      , url, expectedSourceHostId, expectedSourceBootId, browserHostId, browserBootId,
    ] = bootstrapMatch;
    process.stdout.write(`distributed Signal URL ${url}\n`);
    await page.goto("/hosts/browser/distributed-signal.test.html");
    const result = await page.evaluate(async ({
      url, expectedSourceHostId, expectedSourceBootId, browserHostId, browserBootId,
    }) => {
      const { BrowserDomHost } = await import("/hosts/browser/signal-dom-host.mjs");
      const { BrowserWebSocketLine } = await import(
        "/hosts/browser/websocket-line.mjs"
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
      const line = await new BrowserWebSocketLine({
        url,
        maximumMessageBytes: 2048,
        maximumBufferedBytes: 8192,
      }).open();
      const domHost = new BrowserDomHost({
        hostId: browserHostId,
        bootId: browserBootId,
        root: document.querySelector("#browser-sink"),
        maximumReceiptItems: 16,
        maximumReceiptBytes: 144,
      });
      const runtime = await instantiateDistributedBrowserRuntime(wasmBytes, {
        sourceIdentity: { hostId: expectedSourceHostId, bootId: expectedSourceBootId },
        sinkIdentity: { hostId: browserHostId, bootId: browserBootId },
      });
      const run = await runDistributedBrowserRuntime(runtime, line, domHost);
      const closed = await line.closed();
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
    }, {
      url,
      expectedSourceHostId,
      expectedSourceBootId,
      browserHostId,
      browserBootId,
    });
    process.stdout.write(
      `browser receipts=${result.receiptCount} pressure_retries=${result.pressureRetries} ` +
      `retained=${result.retainedValues} in_flight=${result.inFlightItems} ` +
      `capacity_stable=${result.capacityStable}\n`,
    );
    await exited;
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    const summary = await lines.line(1);
    process.stdout.write(`${summary}\n`);
    expect(stderr).toEqual([]);
    expect(summary).toContain("values=16 pressure_retries=1 retained=0 in_flight=0");
    expect(summary).toContain("source_terminal=completed browser_terminal=completed");
    expect(summary).toContain("capacity_stable=true");
    expect(result.status).toBe(1);
    expect(result.receiptCount).toBe(16);
    expect(result.receipts).toHaveLength(16);
    expect(report.hosts.map(({ advertisement }) => advertisement.host_id).sort()).toEqual(
      [expectedSourceHostId, browserHostId].sort(),
    );
    expect(report.lines).toHaveLength(1);
    expect(report.lines[0].offer.line_id).toBe("s4/line/distributed-websocket");
    expect(report.plans).toHaveLength(1);
    expect(report.plays).toHaveLength(1);
    expect(report.plays[0].host_id).toBe(browserHostId);
    expect(report.plays[0].lifecycle).toBe("Completed");
    expect(report.observations.filter(({ kind }) => "ValuePresented" in kind)).toHaveLength(16);
    expect(report.observations.every(({ host_id }) => host_id === browserHostId)).toBe(true);
    expect(new Set(report.observations.filter(({ presentation_id }) => presentation_id)
      .map(({ presentation_id }) => presentation_id)).size).toBe(16);
    expect(new Set(report.observations.map(({ sign_id }) => sign_id)).size).toBe(17);
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
      "signId",
      "connectionId",
      "linkBindingId",
      "baseInstanceId",
    ];
    for (const [index, presentation] of result.presentations.entries()) {
      const receipt = result.receipts[index];
      for (const field of exactReceiptFields) {
        expect(receipt[field], `receipt ${index} ${field}`).toBe(presentation[field]);
      }
      expect(receipt.encoded).toEqual(presentation.encoded);
    }
    expect(result.receipts.every(({ sourceHostId }) =>
      sourceHostId === expectedSourceHostId)).toBe(true);
    expect(result.receipts.every(({ sourceBootId }) =>
      sourceBootId === expectedSourceBootId)).toBe(true);
    expect(result.receipts.every(({ hostId }) => hostId === browserHostId)).toBe(true);
    expect(result.receipts.every(({ bootId }) =>
      bootId === browserBootId)).toBe(true);
    expect(result.receipts.every(({ linkBindingId }) =>
      linkBindingId === "s4/std-browser-link")).toBe(true);
    expect(result.receipts.every(({ baseInstanceId }) =>
      baseInstanceId === "s4/websocket-loopback-instance")).toBe(true);
    expect(new Set(result.presentations.map(({ planId }) => planId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ fragmentId }) => fragmentId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ sourceFragmentId }) => sourceFragmentId)).size)
      .toBe(1);
    expect(new Set(result.presentations.map(({ activePlayId }) => activePlayId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ sourceActivePlayId }) =>
      sourceActivePlayId)).size).toBe(1);
    expect(new Set(result.presentations.map(({ presentationId }) => presentationId)).size).toBe(16);
    expect(new Set(result.presentations.map(({ signId }) => signId)).size).toBe(16);
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
    rmSync(reportPath, { force: true });
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});

test("native Patchbay source and browser peer execute one exact distributed Play", async ({
  page,
}) => {
  const windowed = process.env.CONDUIT_PATCHBAY_NATIVE_WINDOW === "1";
  const source = spawn(
    "target/debug/patchbay-native",
    windowed
      ? ["--distributed-play", "--smoke-exit-after-window"]
      : ["--distributed-play-server"],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
  );
  const stderr = [];
  source.stderr.setEncoding("utf8");
  source.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = lineCollector(source.stdout);
  const exited = processExit(source);
  try {
    const bootstrap = windowed
      ? await lines.matching(/^patchbay distributed url=/)
      : await lines.line(0);
    const match = bootstrap.match(
      windowed
        ? /^patchbay distributed url=(ws:\/\/127\.0\.0\.1:\d+\/conduit) source-host=(\S+) source-boot=(\S+) plan=(\S+)$/
        : /^(ws:\/\/127\.0\.0\.1:\d+\/conduit) source_host=(\S+) source_boot=(\S+) plan=(\S+)$/,
    );
    expect(match).not.toBeNull();
    const [, url, sourceHostId, sourceBootId, planId] = match;
    expect(sourceHostId).toMatch(/^patchbay-native\//);
    expect(sourceBootId).toMatch(/^patchbay-boot\//);
    await page.goto("/hosts/browser/distributed-signal.test.html");
    const result = await page.evaluate(async ({ url, sourceHostId, sourceBootId }) => {
      const { BrowserDomHost } = await import("/hosts/browser/signal-dom-host.mjs");
      const { BrowserWebSocketLine } = await import(
        "/hosts/browser/websocket-line.mjs"
      );
      const {
        instantiateDistributedBrowserRuntime,
        runDistributedBrowserRuntime,
      } = await import("/hosts/browser/distributed-signal-runtime.mjs");
      const wasmBytes = await fetch(
        "/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
      ).then((response) => response.arrayBuffer());
      const line = await new BrowserWebSocketLine({
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
      const runtime = await instantiateDistributedBrowserRuntime(wasmBytes, {
        sourceIdentity: { hostId: sourceHostId, bootId: sourceBootId },
      });
      const run = await runDistributedBrowserRuntime(runtime, line, domHost);
      return { run, receipts: domHost.receipts(), closed: await line.closed() };
    }, { url, sourceHostId, sourceBootId });
    await exited;
    const summary = await lines.matching(/^summary /);
    if (windowed) {
      expect(await lines.matching(/^patchbay distributed-rendered status=completed$/))
        .toBe("patchbay distributed-rendered status=completed");
      expect(await lines.matching(/^patchbay manifestation=\S+ renderer-plan=\S+ renderer-play=\S+ lifecycle=available$/))
        .toMatch(/lifecycle=available$/);
    }
    expect(stderr).toEqual([]);
    expect(summary).toContain(`summary plan=${planId}`);
    expect(summary).toContain("source_terminal=completed browser_terminal=completed");
    expect(result.run.status).toBe(1);
    expect(result.run.receiptCount).toBe(16);
    expect(result.closed).toEqual({ ok: true, code: 1000, reason: "conduit-terminal" });
    expect(result.receipts).toHaveLength(16);
    expect(result.receipts.every(({ planId: receiptPlan }) => receiptPlan === planId)).toBe(true);
    expect(result.receipts.every(({ sourceHostId: host }) => host === sourceHostId)).toBe(true);
    expect(result.receipts.every(({ sourceBootId: boot }) => boot === sourceBootId)).toBe(true);
    expect(new Set(result.receipts.map(({ sourceActivePlayId }) => sourceActivePlayId)).size)
      .toBe(1);
    expect(new Set(result.receipts.map(({ activePlayId }) => activePlayId)).size).toBe(1);
  } finally {
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});

test("a broken live link after four delivered values fails with retained exact receipts", async ({
  page,
}) => {
  const source = spawn(
    "target/debug/conduit",
    [
      "run",
      "fixtures/forms/signal-demo.conduit",
      "--execution-fixture",
      "std-browser-line",
    ],
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
    const bootstrap = await lines.line(0);
    const match = bootstrap.match(
      /^browser_product url=(ws:\/\/127\.0\.0\.1:\d+\/conduit) source_host=(\S+) source_boot=(\S+) browser_host=(\S+) browser_boot=(\S+) plan=(\S+)$/,
    );
    expect(match).not.toBeNull();
    const [, url, sourceHostId, sourceBootId, browserHostId, browserBootId] = match;
    await page.goto("/hosts/browser/distributed-signal.test.html");
    const result = await page.evaluate(async ({
      url, sourceHostId, sourceBootId, browserHostId, browserBootId,
    }) => {
      const { BrowserDomHost } = await import("/hosts/browser/signal-dom-host.mjs");
      const { BrowserWebSocketLine } = await import(
        "/hosts/browser/websocket-line.mjs"
      );
      const {
        instantiateDistributedBrowserRuntime,
        runDistributedBrowserRuntime,
      } = await import("/hosts/browser/distributed-signal-runtime.mjs");
      const wasmBytes = await fetch(
        "/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
      ).then((response) => response.arrayBuffer());
      const line = await new BrowserWebSocketLine({
        url,
        maximumMessageBytes: 2048,
        maximumBufferedBytes: 8192,
      }).open();
      const domHost = new BrowserDomHost({
        hostId: browserHostId,
        bootId: browserBootId,
        root: document.querySelector("#browser-sink"),
        maximumReceiptItems: 16,
        maximumReceiptBytes: 144,
      });
      const runtime = await instantiateDistributedBrowserRuntime(wasmBytes, {
        sourceIdentity: { hostId: sourceHostId, bootId: sourceBootId },
        sinkIdentity: { hostId: browserHostId, bootId: browserBootId },
      });
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
        await runDistributedBrowserRuntime(runtime, line, breakingDomHost);
      } catch (error) {
        failure = { code: error.code ?? null, message: error.message };
      }
      return {
        failure,
        closed: await line.closed(),
        receipts: domHost.receipts(),
      };
    }, { url, sourceHostId, sourceBootId, browserHostId, browserBootId });
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

test("browser page loss before Play reaches the bounded product readiness refusal", async ({
  page,
}) => {
  const source = spawn(
    "target/debug/conduit",
    [
      "run",
      "fixtures/forms/signal-demo.conduit",
      "--execution-fixture",
      "std-browser-line",
    ],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
  );
  const stderr = [];
  source.stderr.setEncoding("utf8");
  source.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = lineCollector(source.stdout);
  const exitOutcome = processExit(source).then(
    () => ({ ok: true }),
    (error) => ({ ok: false, detail: error.message }),
  );
  try {
    expect(await lines.line(0)).toMatch(/^browser_product url=ws:\/\/127\.0\.0\.1:/);
    await page.goto("/hosts/browser/distributed-signal.test.html");
    await page.close();
    const exit = await exitOutcome;
    expect(exit.ok).toBe(false);
    expect(exit.detail).toContain("distributed source exit code=1");
    expect(stderr.join("")).toContain("peer-before-Play: AcceptDeadline");
  } finally {
    lines.close();
    if (source.exitCode === null) source.kill("SIGTERM");
  }
});
