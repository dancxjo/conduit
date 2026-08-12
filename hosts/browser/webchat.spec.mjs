import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";

let server;
let websocketUrl;
let serverOutput = "";

test.beforeAll(async () => {
  server = spawn("target/debug/webchat-server", ["127.0.0.1:0"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  websocketUrl = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("webchat server did not become ready")), 10_000);
    const inspect = (chunk) => {
      serverOutput += chunk.toString();
      const match = serverOutput.match(/external-websocket-ready address=([^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(`ws://${match[1]}`);
      }
    };
    server.stdout.on("data", inspect);
    server.stderr.on("data", inspect);
    server.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`webchat server exited before ready (${code})\n${serverOutput}`));
    });
  });
});

test.afterAll(async () => {
  if (server?.exitCode === null) server.kill("SIGTERM");
});

test("two native browser clients exchange bounded chat through planned kernels", async ({ browser }) => {
  const context = await browser.newContext();
  const pageA = await context.newPage();
  const pageB = await context.newPage();
  const target = `/hosts/browser/webchat.test.html?ws=${encodeURIComponent(websocketUrl)}`;

  await pageA.goto(target);
  await expect(pageA.getByRole("status")).toHaveText("connected");
  await pageB.goto(target);
  await expect(pageB.getByRole("status")).toHaveText("connected");

  await pageA.getByLabel("Message").fill("hello from A");
  await pageA.getByRole("button", { name: "Send" }).click();
  await expect(pageA.getByRole("listitem")).toHaveText(["hello from A"]);
  await expect(pageB.getByRole("listitem")).toHaveText(["hello from A"]);

  await pageB.getByLabel("Message").fill("hello from B");
  await pageB.getByRole("button", { name: "Send" }).click();
  await expect(pageA.getByRole("listitem")).toHaveText(["hello from A", "hello from B"]);
  await expect(pageB.getByRole("listitem")).toHaveText(["hello from A", "hello from B"]);

  await pageA.evaluate(() => globalThis.__webchat.disconnect());
  await expect(pageA.getByRole("status")).toHaveText("disconnected");
  await pageB.getByLabel("Message").fill("remaining peer continues");
  await pageB.getByRole("button", { name: "Send" }).click();
  await expect(pageB.getByRole("listitem")).toHaveText([
    "hello from A",
    "hello from B",
    "remaining peer continues",
  ]);

  const proofA = await pageA.evaluate(() => globalThis.__webchat.proof());
  const proofB = await pageB.evaluate(() => globalThis.__webchat.proof());
  for (const proof of [proofA, proofB]) {
    expect(proof.capacityStable).toBe(true);
    expect(proof.requestCount).toBeGreaterThan(4);
    for (const identity of [
      "source=", "checked=", "expanded=", "plan=", "fragment=", "play=",
      "placement=", "operation=", "implementation=", "host-operation=",
    ]) {
      expect(proof.identity).toContain(identity);
    }
    expect(proof.identity).not.toContain("hello from");
  }
  const identityField = (proof, name) => proof.identity.match(new RegExp(`${name}=([^ ]+)`))?.[1];
  expect(identityField(proofA, "host")).toBeTruthy();
  expect(identityField(proofB, "host")).toBeTruthy();
  expect(identityField(proofA, "host")).not.toBe(identityField(proofB, "host"));
  expect(identityField(proofA, "boot")).not.toBe(identityField(proofB, "boot"));
  expect(proofA.disconnected).toBe(true);
  expect(proofB.disconnected).toBe(false);

  await pageB.evaluate(() => globalThis.__webchat.disconnect());
  await expect(pageB.getByRole("status")).toHaveText("disconnected");
  await expect.poll(() => server.exitCode).toBe(0);
  expect(serverOutput).not.toContain("hello from");
  expect(serverOutput).not.toContain("remaining peer continues");
  await context.close();
});
