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
  const admissionServer = spawn("target/debug/browser-admission-probe", [], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let admissionOutput = "";
  const admissionUrl = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("admission probe did not become ready")), 10_000);
    const inspect = (chunk) => {
      admissionOutput += chunk.toString();
      const match = admissionOutput.match(/^(ws:\/\/[^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    admissionServer.stdout.on("data", inspect);
    admissionServer.stderr.on("data", inspect);
    admissionServer.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`admission probe exited before ready (${code})\n${admissionOutput}`));
    });
  });
  const context = await browser.newContext();
  const pageA = await context.newPage();
  const pageB = await context.newPage();
  const target = `/hosts/browser/webchat.test.html?ws=${encodeURIComponent(websocketUrl)}`;
  const admittedTarget = `${target}&body=${encodeURIComponent(admissionUrl)}`;

  await pageA.goto(admittedTarget);
  await expect(pageA.getByRole("status")).toHaveText("connected");
  await expect.poll(() => pageA.evaluate(() => globalThis.__webchat.bodyAdmission.state())).toBe("admitted");
  await expect.poll(() => admissionServer.exitCode).toBe(0);
  expect(admissionOutput).toContain("admitted body=");
  expect(admissionOutput).toContain("candidates=1 members=1");
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
  const candidates = await Promise.all([pageA, pageB].map((page) => page.evaluate(() => ({
    hostId: globalThis.__webchat.admissionCandidate.hostId,
    bootId: globalThis.__webchat.admissionCandidate.bootId,
    verifyingKey: Array.from(globalThis.__webchat.admissionCandidate.verifyingKey),
    advertisement: globalThis.__webchat.admissionCandidate.advertisement,
  }))));
  expect(candidates[0].hostId).not.toBe(candidates[1].hostId);
  expect(candidates[0].bootId).not.toBe(candidates[1].bootId);
  expect(candidates[0].verifyingKey).toHaveLength(32);
  expect(candidates[0].verifyingKey).not.toEqual(candidates[1].verifyingKey);
  for (const candidate of candidates) {
    expect(candidate.advertisement.host_id).toBe(candidate.hostId);
    expect(candidate.advertisement.boot_id).toBe(candidate.bootId);
    expect(candidate.advertisement.offer_generation).toBe(1);
    expect(candidate.advertisement.capabilities).toHaveLength(3);
    expect(candidate.advertisement.resources).toHaveLength(2);
  }
  const challengeFor = (candidate, suffix) => ({
    admission_id: `admission/browser-${suffix}`,
    body_id: "body/browser-live",
    candidate_id: `candidate/browser-${suffix}`,
    host_id: candidate.hostId,
    boot_id: candidate.bootId,
    offer_generation: 1,
    nonce: Array(32).fill(suffix === "a" ? 17 : 23),
    issued_at_millis: 1_000,
    expires_at_millis: 2_000,
  });
  const signatures = await Promise.all([
    pageA.evaluate((challenge) => Array.from(globalThis.__webchat.admissionCandidate.prove(challenge)), challengeFor(candidates[0], "a")),
    pageB.evaluate((challenge) => Array.from(globalThis.__webchat.admissionCandidate.prove(challenge)), challengeFor(candidates[1], "b")),
  ]);
  expect(signatures[0]).toHaveLength(64);
  expect(signatures[0]).not.toEqual(signatures[1]);
  const staleProof = await pageA.evaluate((challenge) => {
    try {
      globalThis.__webchat.admissionCandidate.prove(challenge);
      return "accepted";
    } catch (error) {
      return String(error);
    }
  }, { ...challengeFor(candidates[0], "stale"), boot_id: "browser-boot/stale" });
  expect(staleProof).toContain("admission proof failed");
  expect(proofA.disconnected).toBe(true);
  expect(proofB.disconnected).toBe(false);

  await pageB.evaluate(() => globalThis.__webchat.disconnect());
  await expect(pageB.getByRole("status")).toHaveText("disconnected");
  await expect.poll(() => server.exitCode).toBe(0);
  expect(serverOutput).not.toContain("hello from");
  expect(serverOutput).not.toContain("remaining peer continues");
  await context.close();
});
