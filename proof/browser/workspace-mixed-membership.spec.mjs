import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { birthWorkspaceEvidence } from "./workspace-body-evidence-support.mjs";

test("one Workspace Body later admits independent browser Hosts", async ({ page, browser }) => {
  const temporary = await mkdtemp(join(tmpdir(), "conduit-rosehip-house-"));
  const processes = [];
  const contexts = [];
  try {
    const { current, evidencePath } = await birthWorkspaceEvidence(
      page, temporary, "Rosehip House", ["Button Across the Room"], "rosehip-house.json",
    );
    const bodyId = current.body_id;
    expect(bodyId).toMatch(/^[0-9a-f]{64}$/);
    const capstone = spawn("target/debug/browser-parts-capstone", ["--body-evidence", evidencePath], {
      cwd: new URL("../..", import.meta.url).pathname,
      stdio: ["ignore", "pipe", "pipe"],
    });
    processes.push(capstone);
    let output = "";
    const invitation = await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error(`Rosehip admission did not become ready\n${output}`)), 10_000);
      const inspect = chunk => {
        output += chunk.toString();
        const body = output.match(/body_url=([^\s]+)/)?.[1];
        const spawnHex = output.match(/spawn_hex=([^\s]+)/)?.[1];
        if (body && spawnHex) { clearTimeout(timeout); resolve({ body, spawnHex }); }
      };
      capstone.stdout.on("data", inspect);
      capstone.stderr.on("data", inspect);
      capstone.once("exit", code => { clearTimeout(timeout); if (code !== 0) reject(new Error(`Rosehip admission exited (${code})\n${output}`)); });
    });
    const ambientChat = await startWebchatServer();
    const spawnedChat = await startWebchatServer();
    processes.push(ambientChat.process, spawnedChat.process);
    const context = await browser.newContext();
    contexts.push(context);
    const ambientTarget = `/proof/browser/webchat.test.html?ws=${encodeURIComponent(ambientChat.url)}&body=${encodeURIComponent(invitation.body)}`;
    const first = await context.newPage();
    const second = await context.newPage();
    await first.goto(ambientTarget);
    await expect.poll(() => first.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting")).toBe("admitted");
    await second.goto(ambientTarget);
    await expect.poll(() => second.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting")).toBe("admitted");
    const third = await context.newPage();
    await third.goto(`/proof/browser/webchat.test.html?ws=${encodeURIComponent(spawnedChat.url)}#body=${encodeURIComponent(invitation.body)}&spawn_hex=${invitation.spawnHex}`);
    await expect.poll(async () => {
      if (capstone.exitCode !== null) throw new Error(`Rosehip admission exited (${capstone.exitCode})\n${output}`);
      return third.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting");
    }).toBe("admitted");
    const replay = await context.newPage();
    await replay.goto(`/proof/browser/webchat.test.html?ws=${encodeURIComponent(spawnedChat.url)}#body=${encodeURIComponent(invitation.body)}&spawn_hex=${invitation.spawnHex}`);
    await expect.poll(() => replay.evaluate(() => globalThis.__webchat?.bodyAdmission.state() ?? "starting")).toBe("refused:replay");
    await first.close();
    await expect.poll(() => {
      if (capstone.exitCode && capstone.exitCode !== 0) throw new Error(output);
      return capstone.exitCode;
    }).toBe(0);
    const receipt = output.split("\n").filter(line => line.startsWith("{")).map(line => JSON.parse(line)).at(-1);
    expect(receipt).toMatchObject({
      schema: "conduit.body/mixed-membership-capstone@1",
      body_id: bodyId,
      friendly_name: "Rosehip House",
      birth_source: "validated-creche-biography",
      browser_parts: 3,
      active_plan_unchanged_by_join: true,
      replacement_plan_distinct: true,
    });
    expect(receipt.parts).toHaveLength(4);
    expect(receipt.parts.filter(({ presentation_state }) => presentation_state === "offline")).toHaveLength(1);
    expect(receipt.declared_bounds.body_parts).toBe(16);
  } finally {
    for (const context of contexts) await context.close().catch(() => {});
    for (const process of processes) if (process.exitCode === null) process.kill("SIGTERM");
    await rm(temporary, { recursive: true, force: true });
  }
});

async function startWebchatServer() {
  const process = spawn("target/debug/webchat-server", ["127.0.0.1:0"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`webchat server did not become ready\n${output}`)), 10_000);
    const inspect = chunk => {
      output += chunk.toString();
      const match = output.match(/external-websocket-ready address=([^\s]+)/);
      if (match) { clearTimeout(timeout); resolve(`ws://${match[1]}`); }
    };
    process.stdout.on("data", inspect);
    process.stderr.on("data", inspect);
    process.once("exit", code => { clearTimeout(timeout); reject(new Error(`webchat server exited (${code})\n${output}`)); });
  });
  return { process, url };
}
