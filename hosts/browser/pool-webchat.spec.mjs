import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";

let server;
let lineUrl;
let serverOutput = "";

test.beforeAll(async () => {
  server = spawn("target/debug/pool-webchat-server", ["127.0.0.1:0"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  lineUrl = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("pool webchat server did not become ready")), 10_000);
    const inspect = (chunk) => {
      serverOutput += chunk.toString();
      const match = serverOutput.match(/pool-webchat-ready address=([^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(`ws://${match[1]}`);
      }
    };
    server.stdout.on("data", inspect);
    server.stderr.on("data", inspect);
    server.once("exit", (code) => reject(new Error(`pool webchat server exited before ready (${code})\n${serverOutput}`)));
  });
});

test.afterAll(async () => {
  if (server?.exitCode === null) server.kill("SIGTERM");
});

test("dynamic browser peers join broadcast leave and continue through the planned pool", async ({ browser, request }) => {
  const source = await request.get("/examples/pool-webchat.conduit");
  const authored = await source.text();
  expect(authored).toContain("pool peers: chat/peer(size = 32)");
  expect(authored).toContain("flow/merge(peers)");
  expect(authored).toContain("flow/fan(peers)");
  expect(authored).not.toMatch(/websocket|socket|address|net\//i);

  const context = await browser.newContext();
  const pageA = await context.newPage();
  const pageB = await context.newPage();
  const target = `/hosts/browser/pool-webchat.test.html?line=${encodeURIComponent(lineUrl)}`;
  await pageA.goto(target);
  await expect(pageA.getByRole("status")).toHaveText("joined");
  await pageB.goto(target);
  await expect(pageB.getByRole("status")).toHaveText("joined");

  await pageA.getByLabel("Message").fill("hello from A");
  await pageA.getByRole("button", { name: "Send" }).click();
  await expect(pageA.getByRole("listitem")).toHaveText(["hello from A"]);
  await expect(pageB.getByRole("listitem")).toHaveText(["hello from A"]);

  await pageB.getByLabel("Message").fill("hello from B");
  await pageB.getByRole("button", { name: "Send" }).click();
  await expect(pageA.getByRole("listitem")).toHaveText(["hello from A", "hello from B"]);
  await expect(pageB.getByRole("listitem")).toHaveText(["hello from A", "hello from B"]);

  await pageA.evaluate(() => globalThis.__poolWebchat.leave());
  await expect(pageA.getByRole("status")).toHaveText("left");
  await pageB.getByLabel("Message").fill("remaining peer continues");
  await pageB.getByRole("button", { name: "Send" }).click();
  await expect(pageB.getByRole("listitem")).toHaveText([
    "hello from A",
    "hello from B",
    "remaining peer continues",
  ]);
  expect(await pageA.evaluate(() => globalThis.__poolWebchat.proof())).toMatchObject({
    sent: 1,
    received: 2,
    disconnected: true,
  });
  expect(await pageB.evaluate(() => globalThis.__poolWebchat.proof())).toMatchObject({
    sent: 2,
    received: 3,
    disconnected: false,
  });

  await pageB.evaluate(() => globalThis.__poolWebchat.leave());
  await expect(pageB.getByRole("status")).toHaveText("left");
  await expect.poll(() => server.exitCode).toBe(0);
  expect(serverOutput).toContain("pool-webchat-complete");
  expect(serverOutput).toContain("population=0");
  expect(serverOutput).toMatch(/source=[^\s]+ plan=[0-9a-f]+ pool=pool-webchat\/peers line=websocket/);
  expect(serverOutput).not.toContain("hello from");
  expect(serverOutput).not.toContain("remaining peer continues");
  await context.close();
});
