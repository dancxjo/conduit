import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { openCrecheStep, reviewAndBirth } from "./creche-test-actions.mjs";
import { startStaticProduct } from "./static-product-server.mjs";

let entrance;

test.beforeEach(async () => {
  entrance = await startStaticProduct(process.env.CONDUIT_CRECHE_RENDEZVOUS_PRODUCT ?? "target/creche-rendezvous-product", "/conduit/workspace/");
});

test.afterEach(() => entrance?.child.kill());

test("a rendezvous code admits one already-running raw Host through the web Crèche", async ({ page }) => {
  const installed = await startInstalledHost();
  const running = spawn("target/debug/conduit", ["host", "rendezvous", "--state-dir", installed.stateDir, "--timeout-seconds", "30"], {
    cwd: installed.root,
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const code = await rendezvousCode(running);
    await page.goto(entrance.url);
    await expect(page.locator("#host-state")).toHaveText("Crèche ready");
    await reviewAndBirth(page);
    await openCrecheStep(page, "3. Physical Host");
    const runner = page.locator(".physical-host-runner");
    await runner.locator('[data-application-key="physical-target"]').selectOption("std/x86_64/computer");
    await runner.locator('[data-application-key="physical-mode"]').selectOption("attach-running");
    const input = runner.getByLabel("Running host rendezvous code");
    await input.fill(code);
    await input.press("Tab");
    await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).toContainText("attachment");

    await runner.getByRole("button", { name: "Bind Body invitation" }).click();
    await expect(runner.locator('[data-application-key="physical-stage-bind"]')).not.toContainText("waiting");
    await runner.getByRole("button", { name: "Realize selected host" }).click();
    await expect(runner.locator('[data-application-key="physical-stage-realize"]')).toContainText("InvitationDelivered");
    await runner.getByRole("button", { name: "Observe Boot and join" }).click();
    await expect(runner.locator('[data-application-key="physical-stage-observe"]')).not.toContainText("waiting");
    await runner.getByRole("button", { name: "Admit Part and offers" }).click();
    await expect(runner.locator('[data-application-key="physical-stage-admit"]')).toContainText("revision");
    await expect(runner.locator('[data-application-slot="physical-status"]')).toContainText("Physical Part admitted");

    const evidence = JSON.parse((await runner.locator(".physical-evidence details code").allTextContents()).join(""));
    expect(evidence).toMatchObject({
      target: { id: "std/x86_64/computer" },
      intention: { mode: "attach-running", supported: true },
      obtainment: { line_id: "conduit-line/loopback-websocket@1", membership_claimed: false },
      realization: { terminal: "InvitationDelivered", membership_claimed: false },
      admission: { disposition: "admitted" },
    });
    await expectProcessSuccess(running);
  } finally {
    try {
      await stopProcess(running);
    } finally {
      await installed.close();
    }
  }
});

async function startInstalledHost() {
  const root = fileURLToPath(new URL("../..", import.meta.url));
  const stateDir = await mkdtemp(join(tmpdir(), "conduit-creche-rendezvous-"));
  let service;
  const close = async () => {
    await stopProcess(service);
    await rm(stateDir, { recursive: true, force: true });
  };
  try {
    // Installation metadata is a fixture; admission is produced by the live service.
    const productExecutable = join(stateDir, "reviewed-host-image");
    await writeFile(productExecutable, "conduit creche rendezvous proof image");
    await writeFile(join(stateDir, "installation.json"), JSON.stringify({
      schema: "conduit.install/durable-host@1",
      host_id: `host/creche-proof/${randomUUID()}`,
      release_source_identity: "commit:creche-proof",
      release_bundle_sha256: `sha256:${"a".repeat(64)}`,
      product_executable: productExecutable,
      body_state: null,
      joined_body_state: null,
    }));
    await writeFile(join(stateDir, "control.token"), Buffer.alloc(32, 7));
    service = spawn("target/debug/conduit", ["host", "service", "run", "--state-dir", stateDir], {
      cwd: root,
      stdio: ["ignore", "pipe", "pipe"],
    });
    await processOutput(service, / is running/, "durable host service");
    return { root, stateDir, close };
  } catch (error) {
    await close();
    throw error;
  }
}

function stopProcess(child) {
  if (!child?.pid || child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const finish = error => {
      clearTimeout(killTimeout);
      clearTimeout(exitTimeout);
      child.off("exit", onExit);
      if (error) reject(error);
      else resolve();
    };
    const onExit = () => finish();
    const killTimeout = setTimeout(() => child.kill("SIGKILL"), 2_000);
    const exitTimeout = setTimeout(() => finish(new Error("Host process did not exit during cleanup")), 5_000);
    child.once("exit", onExit);
    child.kill();
  });
}

function rendezvousCode(child) {
  return processOutput(child, /Rendezvous code: (C1-WS-[0-9A-F-]+)/, "rendezvous code")
    .then(match => match[1]);
}

function processOutput(child, pattern, label) {
  let output = "";
  return new Promise((resolve, reject) => {
    const finish = (error, match) => {
      clearTimeout(timeout);
      child.stdout.off("data", inspect);
      child.stderr.off("data", inspect);
      child.off("exit", onExit);
      child.off("error", onError);
      if (error) reject(error);
      else resolve(match);
    };
    const inspect = chunk => {
      output += chunk.toString();
      const match = output.match(pattern);
      if (match) finish(null, match);
    };
    const onExit = status => finish(new Error(`${label} exited before readiness (${status})\n${output}`));
    const onError = error => finish(error);
    const timeout = setTimeout(() => finish(new Error(`${label} was not ready\n${output}`)), 10_000);
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", onExit);
    child.once("error", onError);
  });
}

function expectProcessSuccess(child) {
  if (child.exitCode !== null) return Promise.resolve(expect(child.exitCode).toBe(0));
  return new Promise((resolve, reject) => child.once("exit", (status) => {
    try {
      expect(status).toBe(0);
      resolve();
    } catch (error) {
      reject(error);
    }
  }));
}
