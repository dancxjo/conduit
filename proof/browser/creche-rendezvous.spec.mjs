import { spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { processOutput, preparationState } from "./process-output.mjs";
import { prepareInstalledHostEncounter } from "./installed-host-encounter.mjs";
import { openWorkspaceMachineRunner } from "./workspace-machine-test-actions.mjs";
import { startStaticProduct } from "./static-product-server.mjs";

const fixtureSources = Object.fromEntries(await Promise.all([
  "creche-rendezvous.spec.mjs", "process-output.mjs", "installed-host-encounter.mjs",
].map(async name => [name, createHash("sha256")
  .update(await readFile(fileURLToPath(new URL(name, import.meta.url)))).digest("hex")])));
let encounter;

test.beforeEach(async ({}, testInfo) => {
  const state = preparationState(testInfo.timeout);
  encounter = state;
  state.entrance = await startStaticProduct(process.env.CONDUIT_CRECHE_RENDEZVOUS_PRODUCT ?? "target/creche-rendezvous-product", "/conduit/workspace/");
});

test.afterEach(async ({}, testInfo) => {
  const state = encounter;
  state.entrance?.child.kill();
  const results = await Promise.allSettled([state.retire()]);
  const diagnostics = testInfo.outputPath("encounter-preparation.json");
  await writeFile(diagnostics, JSON.stringify({ fixture: import.meta.url, source_sha256: fixtureSources, timeout_ms: testInfo.timeout,
    elapsed_ms: performance.now() - state.started, records: state.records }, null, 2));
  await testInfo.attach("encounter-preparation", { contentType: "application/json", path: diagnostics });
  const errors = results.filter(result => result.status === "rejected").map(result => result.reason);
  if (errors.length) throw new AggregateError([...(testInfo.error ? [testInfo.error] : []), ...errors], "Encounter cleanup failed");
});

test("a rendezvous code admits one already-running raw Host through the web Crèche", async ({ page }) => {
  const state = encounter;
  state.cancel = () => page.close();
  const { installed, browser: runner } = await prepareInstalledHostEncounter(
    () => state.run("host", () => startInstalledHost(state)),
    () => state.run("browser", () => openWorkspaceMachineRunner(page, state.entrance)),
  );
  let running, failure;
  const stopRunning = state.own(() => stopProcess(running));
  try {
    state.check();
    running = spawn("target/debug/conduit", ["host", "rendezvous", "--state-dir", installed.stateDir, "--timeout-seconds", "30"], {
      cwd: installed.root,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const code = await rendezvousCode(running, state);
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
    const evidence = JSON.parse((await runner.locator(".physical-evidence details code").allTextContents()).join(""));
    expect(evidence).toMatchObject({
      target: { id: "std/x86_64/computer" },
      intention: { mode: "attach-running", supported: true },
      obtainment: { line_id: "conduit-line/loopback-websocket@1", membership_claimed: false },
      realization: { terminal: "InvitationDelivered", membership_claimed: false },
    });
    // Successful admission returns the current Workspace to its membership panel.
    const before = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership.revision);
    await runner.getByRole("button", { name: "Admit Part and offers" }).click();
    await expect(page.locator(".member-card")).toHaveCount(2);
    // Keep the full host inventory in the browser; only transfer assertion inputs.
    const after = await page.evaluate(hostId => {
      const snapshot = globalThis.__conduitWorkspace.evidence();
      const membership = snapshot.evidence.membership;
      const offer = snapshot.current_host_offers.find(item => item.host_id === hostId);
      return {
        revision: membership.revision,
        added: membership.parts.find(part => part.current?.host_id === hostId),
        offer: offer && {
          host_id: offer.host_id,
          boot_id: offer.boot_id,
          capability_count: offer.capabilities.length,
        },
      };
    }, evidence.observation.host_id);
    expect(after.revision).toBeGreaterThan(before);
    const added = after.added;
    expect(added).toMatchObject({
      state: "Admitted",
      current: { host_id: evidence.observation.host_id, boot_id: evidence.observation.boot_id },
    });
    const offer = after.offer;
    expect(offer).toMatchObject({ host_id: evidence.observation.host_id, boot_id: evidence.observation.boot_id });
    expect(offer.capability_count).toBeGreaterThan(0);
    expect(evidence.observation.advertisement_summary).toMatchObject({
      host_id: evidence.observation.host_id,
      boot_id: evidence.observation.boot_id,
      capability_count: offer.capability_count,
    });
    expect(evidence.observation.advertisement_summary.json_bytes).toBeGreaterThan(200 * 1024);
    expect(evidence.observation.advertisement_summary.json_sha256).toMatch(/^sha256:[0-9a-f]{64}$/);

    await expectProcessSuccess(running);
  } catch (error) {
    failure = error;
    throw error;
  } finally {
    const results = await Promise.allSettled([stopRunning(), installed.close()]);
    const errors = results.filter(result => result.status === "rejected").map(result => result.reason);
    if (errors.length) throw new AggregateError([...(failure ? [failure] : []), ...errors], "Encounter and cleanup failed");
  }
});

async function startInstalledHost(state) {
  const root = fileURLToPath(new URL("../..", import.meta.url));
  const stateDir = await mkdtemp(join(tmpdir(), "conduit-creche-rendezvous-"));
  let service;
  const close = state.own(async () => {
    await stopProcess(service);
    await rm(stateDir, { recursive: true, force: true });
  });
  try {
    state.check();
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
    state.check();
    service = spawn("target/debug/conduit", ["host", "service", "run", "--state-dir", stateDir], {
      cwd: root,
      stdio: ["ignore", "pipe", "pipe"],
    });
    await processOutput(service, / is running/, "durable host service", state);
    return { root, stateDir, close };
  } catch (error) {
    try { await close(); }
    catch (cleanup) { throw new AggregateError([error, cleanup], "Host preparation and cleanup failed"); }
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

function rendezvousCode(child, state) {
  return processOutput(child, /Rendezvous code: (C1-WS-[0-9A-F-]+)/, "rendezvous code", state)
    .then(match => match[1]);
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
