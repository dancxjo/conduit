import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { startPresenceProbe } from "./browser-presence-support.mjs";

const forms = [
  ["Hello", "hello"], ["Greet", "greet"], ["Count", "count"],
  ["Desk Telegraph", "desk-telegraph"], ["Memory Lantern", "memory-lantern"],
  ["Button to Indicator", "button-across-room"], ["Clock", "clock"],
].flatMap(([label, name]) => ["--form", label, `forms/${name}/main.conduit`]);
let processes = [], temporary;

async function open(page, args) {
  const process = spawn("target/debug/patchbay-html", args, {
    cwd: new URL("../..", import.meta.url).pathname, stdio: ["ignore", "pipe", "pipe"],
  });
  processes.push(process);
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Patchbay start timed out: ${output}`)), 10_000);
    const inspect = chunk => {
      output += chunk;
      const match = output.match(/PATCHBAY_HTML_URL=(http:\/\/127\.0\.0\.1:\d+)/);
      if (match) { clearTimeout(timeout);resolve(match[1]); }
    };
    process.stdout.on("data", inspect);process.stderr.on("data", inspect);
    process.once("exit", code => { clearTimeout(timeout);reject(new Error(`Patchbay exited ${code}: ${output}`)); });
  });
  await page.goto(url);
  await expect(page.locator('[data-application-key="product-status"]')).toContainText("Presentation revision");
  return { process, url };
}

const snapshot = page => page.request.get(new URL("/api/snapshot", page.url()).href).then(response => response.json());

test.afterEach(async () => {
  for (const process of processes) process.kill();
  processes = [];
  if (temporary) await rm(temporary, { recursive: true, force: true });
  temporary = null;
});

async function prepareBody(page) {
  const fixture = await open(page, ["--body-workbench-fixture", "external"]);
  const initial = await snapshot(page);
  temporary = await mkdtemp(join(tmpdir(), "conduit-body-execution-"));
  const original = join(temporary, "original.json"), current = join(temporary, "current.json");
  await writeFile(original, Buffer.from(initial.body_workbench.encoded_evidence));
  fixture.process.kill();
  const editor = await open(page, ["--body-evidence", original, "--external-reader", ...forms]);
  const active = page.locator('#body-workbench-forms [data-application-component="artifact"]');
  const available = page.locator('#body-workbench-available [data-application-component="artifact"]');
  await active.filter({ hasText: "Memory Lantern" }).getByRole("button", { name: "Remove from Body", exact: true }).click();
  await expect(active).toHaveCount(1);
  for (const [label, count] of [["Button to Indicator", 2], ["Clock", 3]]) {
    await available.filter({ hasText: label }).getByRole("button", { name: "Add to Body", exact: true }).click();
    await expect(active).toHaveCount(count);
  }
  const edited = await snapshot(page);
  expect(edited.body_workbench.body_id).toBe(initial.body_workbench.body_id);
  await writeFile(current, Buffer.from(edited.body_workbench.encoded_evidence));
  editor.process.kill();
  const probe = await startPresenceProbe(["--body-evidence", current]);processes.push(probe.process);
  await open(page, ["--body-evidence", current, "--external-reader", "--body-invitation", probe.url, ...forms]);
  await page.getByRole("button", { name: "Join this Body", exact: true }).click();
  await expect(page.locator("#body-membership-status")).toContainText("admitted");
  await expect.poll(async () => (await snapshot(page)).body_host_offer_evidence?.stage).toBe("AdmittedMembership");
  await page.getByRole("button", { name: "Request active Form evidence", exact: true }).click();
  await expect(page.locator("#body-capability-evidence-status")).toContainText("SelfReported evidence");
  await page.getByRole("button", { name: "Plan active Forms on this Host", exact: true }).click();
  await expect(page.locator("#body-capability-evidence-status")).toContainText("Body replanned");
  const proposed = await snapshot(page);
  expect(proposed.body_planning.lifecycle).toBe("AwaitingPlan");
  expect(proposed.body_planning.execution_claims).toBeUndefined();
  return { initial, proposed };
}

test("canonical button, clock, and Desk Telegraph run through one page Body Play", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  const { initial, proposed } = await prepareBody(page);
  await page.getByRole("button", { name: "Start proposed Body Play", exact: true }).click();
  await expect(page.locator("#body-execution-status")).toContainText("Body Play running");
  await expect(page.locator("#body-execution-output")).toContainText("CALLING");
  const running = await snapshot(page);
  const claim = running.body_planning.execution_claims[0];
  expect(running.body_planning.lifecycle).toBe("Playing");
  const retained = JSON.parse(Buffer.from(running.body_workbench.encoded_evidence).toString("utf8"));
  expect(retained.body.state.Awake.wake_id).toBe(running.body_planning.wake_id);
  expect(retained.wakes).toHaveLength(1);
  expect(retained.wakes[0].lifecycle).toBe("Playing");
  expect(retained.wakes[0].plans[0].active_play_id).toBe(claim.play.active_play_id);
  expect(claim.phase).toBe("Started");
  expect(claim.play.plan_id).toBe(proposed.body_planning.current_plan_id);
  expect(claim.play.body_id).toBe(initial.body_workbench.body_id);
  expect(claim.started_reported).toBe(true);
  await expect(page.locator("#body-workbench-action")).toBeDisabled();
  await expect(page.locator("#body-execution-output output")).toHaveCount(3);
  const competing = await page.request.post(new URL("/api/body-execution", page.url()).href, { data: {
    schema: "conduit.patchbay/body-execution-request@1",
    action: { kind: "Claim", plan_id: claim.play.plan_id, host_id: claim.host_id, boot_id: claim.boot_id },
  } });
  expect(competing.status()).toBe(409);
  await page.getByRole("group", { name: "Body Play input", exact: true }).hover();
  await page.mouse.down();
  await expect(page.locator('#body-execution-output [data-presentation-kind="presentation/indicator-state"]')).toHaveText("true");
  await page.mouse.up();
  await expect(page.locator('#body-execution-output [data-presentation-kind="presentation/indicator-state"]')).toHaveText("false");
  await expect(page.locator("#body-execution-status")).toContainText("Body Play completed", { timeout: 10_000 });
  const terminal = await snapshot(page);
  expect(terminal.body_planning.execution_claims).toHaveLength(1);
  expect(terminal.body_planning.execution_claims[0].phase.Terminal.disposition).toBe("completed");
  const exported = await (await page.request.get(new URL("/api/body-evidence", page.url()).href)).json();
  expect(exported).toEqual(retained);
  const evidence = JSON.parse(await page.locator("#body-execution-evidence").textContent());
  expect(evidence.play).toEqual(claim.play);
  expect(evidence.receipt.active_play_id).toBe(claim.play.active_play_id);
  expect(evidence.receipt.manifestation_completions).toBe(7);
  expect(evidence.receipt.timer_completions).toBe(4);
  await page.locator("#body-workbench-action").click();
  await expect(page.locator("#body-workbench-action")).toHaveText("Wake");
  const lulled = await snapshot(page);
  expect(lulled.body_planning.lifecycle).toBe("Lulled");
  const closedHistory = JSON.parse(Buffer.from(lulled.body_workbench.encoded_evidence).toString("utf8"));
  expect(closedHistory.wakes[0].lifecycle).toBe("Lulled");
  const active = page.locator('#body-workbench-forms [data-application-component="artifact"]');
  await active.filter({ hasText: "Button to Indicator" }).getByRole("button", { name: "Remove from Body", exact: true }).click();
  await expect(active).toHaveCount(2);
  await page.locator('#body-workbench-available [data-application-component="artifact"]').filter({ hasText: "Button to Indicator" }).getByRole("button", { name: "Add to Body", exact: true }).click();
  await expect(active).toHaveCount(3);
  await page.getByRole("button", { name: "Request active Form evidence", exact: true }).click();
  await expect(page.locator("#body-capability-evidence-status")).toContainText("SelfReported evidence");
  await page.getByRole("button", { name: "Plan active Forms on this Host", exact: true }).click();
  await expect.poll(async () => (await snapshot(page)).body_planning.wake_id).not.toBe(running.body_planning.wake_id);
  const next = await snapshot(page);
  expect(next.body_workbench.body_id).toBe(initial.body_workbench.body_id);
  expect(next.body_planning.historical_plan_ids[0]).toBe(claim.play.plan_id);
  const nextHistory = JSON.parse(Buffer.from(next.body_workbench.encoded_evidence).toString("utf8"));
  expect(nextHistory.wakes).toHaveLength(2);
  expect(nextHistory.wakes[0]).toEqual(closedHistory.wakes[0]);
  await page.getByRole("button", { name: "Start proposed Body Play", exact: true }).click();
  await expect(page.locator("#body-execution-status")).toContainText("Body Play running");
  const second = await snapshot(page);
  expect(second.body_planning.execution_claims).toHaveLength(2);
  expect(second.body_planning.execution_claims[0].play).toEqual(claim.play);
  expect(second.body_planning.execution_claims[1].play.play_sequence).toBe(2);
  await page.getByRole("button", { name: "Cancel Body Play", exact: true }).click();
  await expect(page.locator("#body-execution-status")).toContainText("Body Play cancelled");
  expect(errors).toEqual([]);
});

test("reopening Awake biography does not claim restored execution accounting", async ({ page }) => {
  await prepareBody(page);
  await page.getByRole("button", { name: "Start proposed Body Play", exact: true }).click();
  await expect(page.locator("#body-execution-status")).toContainText("Body Play running");
  await page.getByRole("button", { name: "Cancel Body Play", exact: true }).click();
  await expect(page.locator("#body-execution-status")).toContainText("Body Play cancelled");
  const prior = await snapshot(page);
  const encoded = Buffer.from(prior.body_workbench.encoded_evidence);
  const archived = JSON.parse(encoded.toString("utf8"));
  expect(archived.body.state.Awake).toBeDefined();
  const saved = join(temporary, "reopen.json");
  await writeFile(saved, encoded);
  processes.at(-1).kill();
  await open(page, ["--body-evidence", saved, "--external-reader", ...forms]);
  const reopened = await snapshot(page);
  expect(reopened.body_workbench.body_id).toBe(prior.body_workbench.body_id);
  expect(Buffer.from(reopened.body_workbench.encoded_evidence)).toEqual(encoded);
  expect(reopened.body_planning ?? null).toBeNull();
  await expect(page.locator("#body-execution-status")).toBeVisible();
  await expect(page.locator("#body-execution-status")).toContainText("no restored execution claim or terminal receipt");
  await expect(page.locator("#body-execution-status")).toContainText("No automatic restart is authorized");
  await expect(page.getByRole("button", { name: "Start proposed Body Play", exact: true })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Cancel Body Play", exact: true })).toBeDisabled();
});

test("cancelling the actual Body Play retains exact terminal evidence", async ({ page }) => {
  await prepareBody(page);
  await page.getByRole("button", { name: "Start proposed Body Play", exact: true }).click();
  await expect(page.locator("#body-execution-status")).toContainText("Body Play running");
  const running = await snapshot(page);
  await page.getByRole("button", { name: "Cancel Body Play", exact: true }).click();
  await expect(page.locator("#body-execution-status")).toContainText("Body Play cancelled");
  const terminal = await snapshot(page);
  expect(terminal.body_planning.execution_claims[0].play).toEqual(running.body_planning.execution_claims[0].play);
  expect(terminal.body_planning.execution_claims[0].phase.Terminal.disposition).toBe("cancelled");
  expect(terminal.body_planning.lifecycle).toBe("Playing");
  await expect(page.locator("#body-execution-output output")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Start proposed Body Play", exact: true })).toBeDisabled();
});

test("cancellation while the claim response is held refuses before Play", async ({ page }) => {
  await prepareBody(page);
  let release, received;
  const held = new Promise(resolve => { received = resolve; });
  const released = new Promise(resolve => { release = resolve; });
  await page.route("**/api/body-execution", async route => {
    if (route.request().postDataJSON().action.kind !== "Claim") return route.continue();
    const response = await route.fetch();
    received();await released;
    await route.fulfill({ response });
  });
  await page.getByRole("button", { name: "Start proposed Body Play", exact: true }).click();
  await held;
  const claimed = await snapshot(page);
  expect(claimed.body_planning.execution_claims[0].phase).toBe("Claimed");
  await page.getByRole("button", { name: "Cancel Body Play", exact: true }).click();
  release();
  await expect.poll(async () => (await snapshot(page)).body_planning.execution_claims[0].phase).toEqual({ RefusedBeforeStart: { reason: "Body start cancelled before Play" } });
  const refused = await snapshot(page);
  expect(refused.body_planning.lifecycle).toBe("AwaitingPlan");
  expect(refused.body_planning.execution_claims[0].started_reported).toBe(false);
  await expect(page.locator("#body-execution-output output")).toHaveCount(0);
});
