import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";

let server;
let websocketUrl;
let serverOutput = "";

async function startWebchatServer() {
  const process = spawn("target/debug/webchat-server", ["127.0.0.1:0"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("webchat server did not become ready")), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/external-websocket-ready address=([^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(`ws://${match[1]}`);
      }
    };
    process.stdout.on("data", inspect);
    process.stderr.on("data", inspect);
    process.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`webchat server exited before ready (${code})\n${output}`));
    });
  });
  return { process, url, output: () => output };
}

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

test("Body-directed fragment admits exactly one browser and replay stays refused", async ({ browser }) => {
  const spawnChat = await startWebchatServer();
  const spawnServer = spawn("target/debug/browser-spawn-probe", [], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const invitation = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("spawn probe did not become ready")), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const body = output.match(/body_url=([^\s]+)/)?.[1];
      const spawnHex = output.match(/spawn_hex=([^\s]+)/)?.[1];
      if (body && spawnHex) {
        clearTimeout(timeout);
        resolve({ body, spawnHex });
      }
    };
    spawnServer.stdout.on("data", inspect);
    spawnServer.stderr.on("data", inspect);
    spawnServer.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`spawn probe failed (${code})\n${output}`));
    });
  });
  const context = await browser.newContext();
  const fragment = `#body=${encodeURIComponent(invitation.body)}&spawn_hex=${invitation.spawnHex}`;
  const target = `/hosts/browser/webchat.test.html?ws=${encodeURIComponent(spawnChat.url)}${fragment}`;
  const admitted = await context.newPage();
  await admitted.goto(target);
  await expect(admitted.getByRole("status")).toHaveText("connected");
  await expect.poll(() => admitted.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting")).toBe("admitted");
  const replay = await context.newPage();
  await replay.goto(target);
  await expect.poll(() => replay.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting")).toBe("refused:replay");
  await expect.poll(() => spawnServer.exitCode).toBe(0);
  expect(output).toContain("spawn_admitted=1 replay_refused=true members=1");
  expect(await admitted.evaluate(() => globalThis.__webchat.admissionCandidate.hostId))
    .not.toBe(await replay.evaluate(() => globalThis.__webchat.admissionCandidate.hostId));
});

test("one native Body presents three mixed browser Parts without mutating its Plan", async ({ browser }) => {
  const ambientChat = await startWebchatServer();
  const spawnedChat = await startWebchatServer();
  const picoPort = process.env.CONDUIT_B9_PICO_LINK_PORT;
  const capstone = spawn("target/debug/browser-parts-capstone", [], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const invitation = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("Parts capstone did not become ready")), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const body = output.match(/body_url=([^\s]+)/)?.[1];
      const spawnHex = output.match(/spawn_hex=([^\s]+)/)?.[1];
      if (body && spawnHex) {
        clearTimeout(timeout);
        resolve({ body, spawnHex });
      }
    };
    capstone.stdout.on("data", inspect);
    capstone.stderr.on("data", inspect);
    capstone.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`Parts capstone failed (${code})\n${output}`));
    });
  });
  const context = await browser.newContext();
  const ambientTarget = `/hosts/browser/webchat.test.html?ws=${encodeURIComponent(ambientChat.url)}&body=${encodeURIComponent(invitation.body)}`;
  const first = await context.newPage();
  await first.goto(ambientTarget);
  await expect.poll(() => first.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting")).toBe("admitted");
  const second = await context.newPage();
  await second.goto(ambientTarget);
  await expect.poll(() => second.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting")).toBe("admitted");

  const fragment = `#body=${encodeURIComponent(invitation.body)}&spawn_hex=${invitation.spawnHex}`;
  const spawnTarget = `/hosts/browser/webchat.test.html?ws=${encodeURIComponent(spawnedChat.url)}${fragment}`;
  const third = await context.newPage();
  await third.goto(spawnTarget);
  await expect.poll(async () => {
    if (capstone.exitCode !== null) throw new Error(`Parts capstone exited (${capstone.exitCode})\n${output}`);
    return third.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting");
  }).toBe("admitted");
  const replay = await context.newPage();
  await replay.goto(spawnTarget);
  await expect.poll(() => replay.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting")).toBe("refused:replay");

  await expect.poll(() => output).toContain("ready_for_offline");
  expect(output.match(/^wants_to_join=/gm)).toHaveLength(2);
  await first.close();
  await expect.poll(() => capstone.exitCode).toBe(0);
  expect(output).toContain(`members=${picoPort ? 5 : 4} browser_parts=3 pico_parts=${picoPort ? 1 : 0} offline=1`);
  expect(output).toContain("replay_refused=true plan_unchanged=true replan_distinct=true cross_host_fragments=2");
  if (picoPort) {
    expect(output).toContain("pico_wants_to_join=");
    expect(output).toMatch(/pico_admitted part=[0-9a-f]+ host=r1\/pico-w boot=conduit-pico-w-signal\/runtime-boot:/);
  }
  const receipt = output.split("\n").filter((line) => line.startsWith("{")).map((line) => JSON.parse(line)).at(-1);
  expect(receipt.schema).toBe("conduit.body/mixed-membership-capstone@1");
  expect(receipt.parts).toHaveLength(picoPort ? 5 : 4);
  expect(new Set(receipt.parts.map(({ part_id: id }) => id)).size).toBe(receipt.parts.length);
  expect(new Set(receipt.parts.map(({ host_id: id }) => id)).size).toBe(receipt.parts.length);
  expect(receipt.active_plan_unchanged_by_join).toBe(true);
  expect(receipt.replacement_plan_distinct).toBe(true);
  expect(receipt.physical_pico_admitted).toBe(Boolean(picoPort));
  console.log(JSON.stringify(receipt));
  const identities = await Promise.all([second, third].map((page) => page.evaluate(() => ({
    host: globalThis.__webchat.admissionCandidate.hostId,
    boot: globalThis.__webchat.admissionCandidate.bootId,
  }))));
  expect(identities[0].host).not.toBe(identities[1].host);
  expect(identities[0].boot).not.toBe(identities[1].boot);
  await context.close();
  if (ambientChat.process.exitCode === null) ambientChat.process.kill("SIGTERM");
  if (spawnedChat.process.exitCode === null) spawnedChat.process.kill("SIGTERM");
});
