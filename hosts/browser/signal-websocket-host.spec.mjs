import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";

const linkPort = process.env.CONDUIT_BROWSER_LINK_PORT ?? "4180";
let server;
let serverExit;

test.beforeAll(async () => {
  server = spawn("./target/debug/conduit-browser-link-host", [linkPort], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  server.stdout.setEncoding("utf8");
  server.stderr.setEncoding("utf8");
  server.stdout.on("data", (chunk) => { stdout += chunk; });
  server.stderr.on("data", (chunk) => { stderr += chunk; });
  serverExit = new Promise((resolve) =>
    server.once("exit", (code) => resolve({ code, stdout, stderr }))
  );
  await Promise.race([
    new Promise((resolve, reject) => {
      server.stdout.on("data", (chunk) => {
        if (chunk.includes("ready websocket=")) resolve();
      });
      server.once("error", reject);
      server.once("exit", (code) => reject(new Error(`link host exited before ready: ${code} ${stderr}`)));
    }),
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error("link host readiness timed out")), 5_000)
    ),
  ]);
});

test.afterAll(() => {
  if (server?.exitCode === null) server.kill("SIGTERM");
});

test("std runtime sends the unchanged signal form to the browser over bounded WebSocket", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });

  await page.goto(`/hosts/browser/signal-websocket-host.test.html?linkPort=${linkPort}`);
  await expect(page.locator("#result")).toHaveText("ok", { timeout: 15_000 });
  await expect(page.locator("#remote-host output")).toHaveCount(16);
  await expect(page.locator("#remote-host output").last()).toHaveAttribute("data-sequence", "15");
  expect(await page.evaluate(() => globalThis.__conduitWebSocketResults)).toEqual({
    status: 1,
    completionCount: 16,
    messageCount: 16,
    receiptCount: 16,
    malformedCode: -9,
  });
  const exit = await Promise.race([
    serverExit,
    new Promise((_, reject) => setTimeout(() => reject(new Error("link host exit timed out")), 2_000)),
  ]);
  expect(exit.code).toBe(0);
  expect(exit.stderr).toBe("");
  expect(exit.stdout).toContain("items=1 bytes=64");
  expect(exit.stdout).toContain("complete transmitted=16");
  expect(failures).toEqual([]);
});
