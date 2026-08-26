import { spawn } from "node:child_process";
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
      return new Promise((resolve, reject) => matchWaiters.push({ pattern, resolve, reject }));
    },
    close() {
      reader.close();
    },
  };
}

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
