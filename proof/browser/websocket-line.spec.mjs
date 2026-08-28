import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function nextLine(lines) {
  return new Promise((resolve, reject) => {
    lines.once("line", resolve);
    lines.once("close", () => reject(new Error("base exited before publishing URL")));
  });
}

function processExit(child) {
  return new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`base exit code=${code} signal=${signal}`));
    });
  });
}

test("actual Chromium and native RFC 6455 lines exchange bounded binary protocol frames", async ({
  page,
}) => {
  const base = spawn(
    "cargo",
    ["run", "--quiet", "-p", "conduit-std-host", "--bin", "websocket-line-probe"],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] },
  );
  const stderr = [];
  base.stderr.setEncoding("utf8");
  base.stderr.on("data", (chunk) => stderr.push(chunk));
  const lines = createInterface({ input: base.stdout });
  const exited = processExit(base);
  try {
    const url = await nextLine(lines);
    expect(url).toMatch(/^ws:\/\/127\.0\.0\.1:\d+\/conduit$/);
    await page.goto("/proof/browser/websocket-line.test.html");
    const result = await page.evaluate(async ({ url }) => {
      const { BrowserWebSocketLine, BrowserWebSocketFailure } = await import(
        "/targets/browser/host/assets/websocket-line.mjs"
      );
      let invalidBinding;
      try {
        new BrowserWebSocketLine({
          url: "ws://0.0.0.0:9/conduit",
          maximumMessageBytes: 1024,
          maximumBufferedBytes: 1024,
        });
      } catch (error) {
        invalidBinding = error.message;
      }
      const line = await new BrowserWebSocketLine({
        url,
        maximumMessageBytes: 1024,
        maximumBufferedBytes: 1024,
      }).open();
      const hello = await line.receiveBinary();
      const helloSend = line.sendBinary(hello);
      const ready = await line.receiveBinary();
      const oversized = line.sendBinary(new Uint8Array(1025));
      const readySend = line.sendBinary(ready);
      const closed = await line.closed();
      return {
        invalidBinding,
        helloMagic: new TextDecoder().decode(hello.slice(0, 4)),
        helloBytes: hello.length,
        readyMagic: new TextDecoder().decode(ready.slice(0, 4)),
        readyBytes: ready.length,
        helloSend,
        readySend,
        oversized,
        closed,
        expectedInvalidBinding: BrowserWebSocketFailure.InvalidBinding,
        expectedOversized: BrowserWebSocketFailure.OversizedMessage,
      };
    }, { url });
    await exited;
    expect(stderr).toEqual([]);
    expect(result.invalidBinding).toBe(result.expectedInvalidBinding);
    expect(result.helloMagic).toBe("CNDS");
    expect(result.readyMagic).toBe("CNDS");
    expect(result.helloBytes).toBeLessThanOrEqual(1024);
    expect(result.readyBytes).toBeLessThan(result.helloBytes);
    expect(result.helloSend).toEqual({ ok: true, byteLength: result.helloBytes });
    expect(result.readySend).toEqual({ ok: true, byteLength: result.readyBytes });
    expect(result.oversized).toEqual({
      ok: false,
      code: result.expectedOversized,
      detail: "1025",
    });
    expect(result.closed).toEqual({ ok: true, code: 1000, reason: "conduit-terminal" });
  } finally {
    lines.close();
    if (base.exitCode === null) base.kill("SIGTERM");
  }
});
