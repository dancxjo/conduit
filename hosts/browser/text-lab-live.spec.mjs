import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function lineCollector(stream) {
  const reader = createInterface({ input: stream });
  const lines = [];
  const waiters = [];
  reader.on("line", (line) => {
    lines.push(line);
    while (waiters.length > 0 && lines.length > waiters[0].index) {
      const waiter = waiters.shift();
      waiter.resolve(lines[waiter.index]);
    }
  });
  return {
    line(index) {
      if (lines.length > index) return Promise.resolve(lines[index]);
      return new Promise((resolve) => waiters.push({ index, resolve }));
    },
    close() {
      reader.close();
    },
  };
}

function processExit(child) {
  return new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`Text Lab server exit code=${code} signal=${signal}`));
    });
  });
}

test("unchanged Text Lab executes both exact Lines through browser WASM", async ({ page }) => {
  const sourceText = readFileSync("examples/text-lab.conduit", "utf8").toLowerCase();
  for (const forbidden of ["browser", "websocket", "127.0.0.1", "host", "line", "address"]) {
    expect(sourceText).not.toContain(forbidden);
  }
  const server = spawn(
    "cargo",
    ["run", "--quiet", "-p", "conduit-std-host", "--bin", "text-lab-live-server"],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
  );
  const stderr = [];
  server.stderr.setEncoding("utf8");
  server.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = lineCollector(server.stdout);
  const exited = processExit(server);
  try {
    const url = await lines.line(0);
    expect(url).toMatch(/^ws:\/\/127\.0\.0\.1:\d+\/conduit$/);
    await page.goto("/hosts/browser/text-lab-live.test.html");
    const result = await page.evaluate(async ({ url }) => {
      const { BrowserWebSocketLine } = await import("/hosts/browser/websocket-line.mjs");
      const { instantiateTextLabLive, runTextLabLive } = await import(
        "/hosts/browser/text-lab-live-runtime.mjs"
      );
      const wasmBytes = await fetch(
        "/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
      ).then((response) => {
        if (!response.ok) throw new Error("Text Lab browser WASM artifact missing");
        return response.arrayBuffer();
      });
      const openLine = () => new BrowserWebSocketLine({
        url,
        maximumMessageBytes: 1024,
        maximumBufferedBytes: 4096,
      }).open();
      const forward = await openLine();
      const runtime = await instantiateTextLabLive(wasmBytes, url);
      const run = await runTextLabLive(runtime, forward, openLine);
      const [forwardClosed, returnClosed] = await Promise.all([
        forward.closed(),
        run.returned.closed(),
      ]);
      document.querySelector("#result").textContent = "complete";
      return {
        status: runtime.api.conduit_browser_text_lab_status(),
        sentFrames: run.sentFrames,
        receivedFrames: run.receivedFrames,
        forwardClosed,
        returnClosed,
      };
    }, { url });
    await exited;
    const summary = await lines.line(1);
    expect(stderr).toEqual([]);
    expect(summary).toBe(
      "text_lab=HELLO values=5 forward_terminal=completed return_terminal=completed",
    );
    expect(result.status).toBe(1);
    expect(result.sentFrames).toBeGreaterThan(0);
    expect(result.receivedFrames).toBeGreaterThan(0);
    expect(result.forwardClosed).toEqual({ ok: true, code: 1000, reason: "conduit-terminal" });
    expect(result.returnClosed).toEqual({ ok: true, code: 1000, reason: "conduit-terminal" });
    await expect(page.locator("#result")).toHaveText("complete");
  } finally {
    lines.close();
    if (server.exitCode === null) server.kill("SIGTERM");
  }
});
