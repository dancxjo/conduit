import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { randomUUID } from "node:crypto";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;
test.beforeEach(async () => { entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
test.afterEach(() => entrance?.child.kill());

test("a second distinct browser Host explicitly joins through one canonical Body invitation", async ({ page, context }) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();

  await expect(page.getByRole("button", { name: "Parts / Hosts", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Parts / Hosts", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Parts and Hosts", exact: true })).toBeVisible();
  await expect(page.locator(".member-card")).toHaveCount(1);
  await expect(page.locator(".member-card")).toContainText("admitted · present · local Host");
  const before = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);

  await page.getByRole("button", { name: "Invite another Host", exact: true }).click();
  await expect(page.getByText("Invitation ready", { exact: true })).toBeVisible();
  const link = await page.locator("[data-invitation-link]").inputValue();
  const afterOffer = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  expect(afterOffer).toEqual(before);
  expect(new URL(link).hash).toContain("body-invitation=");

  const joining = await context.newPage();
  await joining.goto(link);
  await expect(joining.getByText("Body invitation", { exact: true })).toBeVisible();
  await expect(joining.getByText("It grants no Form or effect authority.")).toBeVisible();
  await joining.getByRole("button", { name: "Join this Body", exact: true }).click();

  await expect(joining.locator(".member-card")).toHaveCount(2);
  await expect(page.locator(".member-card")).toHaveCount(2);
  const authority = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  const receiver = await joining.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  expect(authority.revision).toBe(before.revision + 2);
  expect(receiver).toEqual(authority);
  expect(new Set(authority.parts.map(part => part.current.host_id)).size).toBe(2);
  expect(authority.parts.every(part => part.state === "Admitted" && part.current)).toBe(true);
  const authorityOffers = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers);
  const receiverOffers = await joining.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers);
  expect(authorityOffers).toHaveLength(2);
  expect(receiverOffers).toHaveLength(1);
  for (const offer of authorityOffers) {
    const member = authority.parts.find(part => part.current.host_id === offer.host_id);
    expect(offer.boot_id).toBe(member.current.boot_id);
    expect(offer.offer_generation).toBe(member.current.offer_generation);
  }

  await joining.close();
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await page.getByRole("button", { name: "Parts / Hosts", exact: true }).click();
  await expect(page.locator(".member-card")).toHaveCount(2);
  const restored = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  expect(restored.parts.filter(part => part.current)).toHaveLength(1);
  expect(restored.parts.filter(part => !part.current && part.state === "Admitted")).toHaveLength(1);
  const restoredOffers = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers);
  expect(restoredOffers).toHaveLength(1);
  expect(restoredOffers[0].host_id).toBe(restored.parts.find(part => part.current).current.host_id);
});

test("malformed invitation framing is refused without creating a Body", async ({ page }) => {
  await page.goto(`${entrance.url}#body-invitation=not-json`);
  await expect(page.locator("[data-workspace-notice]")).toContainText("Body invitation is malformed");
  expect(await page.evaluate(() => globalThis.__conduitWorkspace)).toBeUndefined();
});

test("an unavailable running Host leaves the current Body and browser Host intact", async ({ page }) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  const before = await page.evaluate(() => globalThis.__conduitWorkspace.evidence());

  await page.getByRole("button", { name: "Parts / Hosts", exact: true }).click();
  await page.getByRole("button", { name: "Connect a running Host", exact: true }).click();
  await page.getByLabel("Running Host rendezvous code", { exact: true }).fill("not-a-rendezvous-code");
  await page.getByRole("button", { name: "Connect Host", exact: true }).click();

  await expect(page.locator("[data-workspace-notice]")).toContainText("not a supported finite Line code");
  expect(await page.evaluate(() => globalThis.__conduitWorkspace.evidence())).toEqual(before);
  await page.getByRole("button", { name: "Back to members", exact: true }).click();
  await expect(page.locator(".member-card")).toHaveCount(1);
  await expect(page.locator(".member-card")).toContainText("admitted · present · local Host");
});

test("an already-running Host joins without displacing the browser Host or its Body", async ({ page }) => {
  const installed = await startInstalledHost();
  const running = spawn("target/debug/conduit", ["host", "rendezvous", "--state-dir", installed.stateDir, "--timeout-seconds", "30"], {
    cwd: installed.root,
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const code = await rendezvousCode(running);
    await page.goto(entrance.url);
    await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
    await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
    await page.getByRole("button", { name: "Birth Body", exact: true }).click();
    const before = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);

    await page.getByRole("button", { name: "Parts / Hosts", exact: true }).click();
    await page.getByRole("button", { name: "Connect a running Host", exact: true }).click();
    await page.getByLabel("Running Host rendezvous code", { exact: true }).fill(code);
    await page.getByRole("button", { name: "Connect Host", exact: true }).click();

    await expect(page.locator(".member-card")).toHaveCount(2);
    await expect(page.locator(".member-card")).toContainText(["admitted · present · local Host", "admitted · present"]);
    const after = await page.evaluate(() => globalThis.__conduitWorkspace.evidence());
    expect(after.evidence.membership.revision).toBe(before.revision + 2);
    expect(after.evidence.membership.parts.every(part => part.state === "Admitted" && part.current)).toBe(true);
    expect(after.current_host_offers).toHaveLength(2);
    await page.reload();
    await expectProcessSuccess(running);
    await page.getByRole("button", { name: "Parts / Hosts", exact: true }).click();
    await expect(page.locator(".member-card")).toHaveCount(2);
    const restored = await page.evaluate(() => globalThis.__conduitWorkspace.evidence());
    expect(restored.evidence.membership.parts.filter(part => part.current)).toHaveLength(1);
    expect(restored.current_host_offers).toHaveLength(1);
  } finally {
    if (running.exitCode === null) running.kill();
    await installed.close();
  }
});

test("loss of a joined Host Line removes only its current offers and keeps the Body", async ({ page }) => {
  const installed = await startInstalledHost();
  const running = spawn("target/debug/conduit", ["host", "rendezvous", "--state-dir", installed.stateDir, "--timeout-seconds", "30"], {
    cwd: installed.root,
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const code = await rendezvousCode(running);
    await page.goto(entrance.url);
    await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
    await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
    await page.getByRole("button", { name: "Birth Body", exact: true }).click();
    const bodyId = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.body_id);

    await page.getByRole("button", { name: "Parts / Hosts", exact: true }).click();
    await page.getByRole("button", { name: "Connect a running Host", exact: true }).click();
    await page.getByLabel("Running Host rendezvous code", { exact: true }).fill(code);
    await page.getByRole("button", { name: "Connect Host", exact: true }).click();
    await expect(page.locator(".member-card")).toHaveCount(2);
    expect(await page.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers.length)).toBe(2);

    running.kill();
    await expect(page.locator("[data-workspace-notice]")).toContainText("Voice stopped; Body Chat remains available");
    const after = await page.evaluate(() => globalThis.__conduitWorkspace.evidence());
    expect(after.evidence.body_id).toBe(bodyId);
    expect(after.evidence.membership.parts.filter(part => part.current)).toHaveLength(1);
    expect(after.evidence.membership.parts.filter(part => !part.current && part.state === "Admitted")).toHaveLength(1);
    expect(after.current_host_offers).toHaveLength(1);
  } finally {
    if (running.exitCode === null) running.kill();
    await installed.close();
  }
});

async function startInstalledHost() {
  const root = new URL("../..", import.meta.url).pathname;
  const stateDir = await mkdtemp(`${tmpdir()}/conduit-workspace-host-`);
  const productExecutable = `${stateDir}/reviewed-host-image`;
  await writeFile(productExecutable, "conduit workspace rendezvous proof image");
  await writeFile(`${stateDir}/installation.json`, JSON.stringify({
    schema: "conduit.install/durable-host@1",
    host_id: `host/workspace-proof/${randomUUID()}`,
    release_source_identity: "commit:workspace-proof",
    release_bundle_sha256: `sha256:${"a".repeat(64)}`,
    product_executable: productExecutable,
    body_state: null,
    joined_body_state: null,
  }));
  await writeFile(`${stateDir}/control.token`, Buffer.alloc(32, 7));
  const service = spawn("target/debug/conduit", ["host", "service", "run", "--state-dir", stateDir], {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
  });
  await processOutput(service, / is running/, "durable Host service");
  return { root, stateDir, close: async () => {
    if (service.exitCode === null) service.kill();
    await new Promise(resolveClose => service.exitCode === null ? service.once("exit", resolveClose) : resolveClose());
    await rm(stateDir, { recursive: true, force: true });
  } };
}

function rendezvousCode(child) {
  return processOutput(child, /Rendezvous code: (C1-WS-[0-9A-F-]+)/, "rendezvous code").then(match => match[1]);
}

function processOutput(child, pattern, label) {
  let output = "";
  return new Promise((resolveOutput, reject) => {
    const timeout = setTimeout(() => reject(new Error(`${label} was not ready\n${output}`)), 10_000);
    const inspect = chunk => {
      output += chunk.toString();
      const match = output.match(pattern);
      if (match) { clearTimeout(timeout); resolveOutput(match); }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", status => { clearTimeout(timeout); reject(new Error(`${label} exited before readiness (${status})\n${output}`)); });
  });
}

function expectProcessSuccess(child) {
  if (child.exitCode !== null) return Promise.resolve(expect(child.exitCode).toBe(0));
  return new Promise((resolve, reject) => child.once("exit", status => {
    try { expect(status).toBe(0); resolve(); }
    catch (error) { reject(error); }
  }));
}
