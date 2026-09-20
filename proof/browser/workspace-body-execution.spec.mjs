import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";
import { startPresenceProbe } from "./browser-presence-support.mjs";

async function birthWorkspaceEvidence(page, temporary, friendlyName, titles, filename) {
  const workspace = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
  await page.goto(workspace.url);
  const birth = page.locator(".body-birth-runner");
  await birth.getByLabel("Friendly Body name", { exact: true }).fill(friendlyName);
  for (const checkbox of await birth.getByRole("checkbox").all()) {
    if (await checkbox.isChecked()) await checkbox.uncheck();
  }
  for (const title of titles) {
    await birth.getByRole("checkbox", { name: title, exact: true }).check();
  }
  await birth.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  const result = await page.evaluate(() => ({
    current: globalThis.__conduitWorkspace.current(),
    evidence: globalThis.__conduitWorkspace.evidence().evidence,
  }));
  const evidencePath = join(temporary, filename);
  await writeFile(evidencePath, `${JSON.stringify(result.evidence)}\n`, "utf8");
  workspace.child.kill();
  return { ...result, evidencePath };
}

async function startWebchatServer() {
  const process = spawn("target/debug/webchat-server", ["127.0.0.1:0"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`webchat server did not become ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/external-websocket-ready address=([^\s]+)/);
      if (match) { clearTimeout(timeout); resolve(`ws://${match[1]}`); }
    };
    process.stdout.on("data", inspect);
    process.stderr.on("data", inspect);
    process.once("exit", (code) => { clearTimeout(timeout); reject(new Error(`webchat server exited (${code})\n${output}`)); });
  });
  return { process, url };
}

test("a Workspace-born canonical workset continues as the same executing Body", async ({ page }) => {
  const temporary = await mkdtemp(join(tmpdir(), "conduit-workspace-execution-"));
  const processes = [];
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  try {
    const { current, evidencePath } = await birthWorkspaceEvidence(
      page,
      temporary,
      "Juniper Workbench",
      ["Button Across the Room", "Clock", "Desk Telegraph"],
      "body.json",
    );
    const bodyId = current.body_id;
    const born = JSON.parse(await readFile(evidencePath, "utf8"));
    expect(born.body_id).toBe(bodyId);
    expect(born.records[0].sign_id).toBeTruthy();
    expect(born.body.workset.forms).toHaveLength(3);
    expect(born.body.workload_revision).toBe(0);

    const probe = await startPresenceProbe(["--body-evidence", evidencePath]);
    processes.push(probe.process);
    const patchbay = spawn("target/debug/patchbay-html", [
      "--body-evidence", evidencePath, "--external-reader", "--body-invitation", probe.url,
      ...[
        ["button-across-room", "button-across-room"],
        ["clock", "clock"],
        ["desk_telegraph", "desk-telegraph"],
      ].flatMap(([name, path]) => ["--form", name, `forms/${path}/main.conduit`]),
    ], { cwd: new URL("../..", import.meta.url).pathname, stdio: ["ignore", "pipe", "pipe"] });
    processes.push(patchbay);
    const url = await new Promise((resolve, reject) => {
      let output = "";
      const timeout = setTimeout(() => reject(new Error(`Patchbay start timed out: ${output}`)), 10_000);
      const inspect = chunk => {
        output += chunk;
        const match = output.match(/PATCHBAY_HTML_URL=(http:\/\/127\.0\.0\.1:\d+)/);
        if (match) { clearTimeout(timeout); resolve(match[1]); }
      };
      patchbay.stdout.on("data", inspect);
      patchbay.stderr.on("data", inspect);
      patchbay.once("exit", code => { clearTimeout(timeout); reject(new Error(`Patchbay exited ${code}: ${output}`)); });
    });
    await page.goto(url);
    const snapshot = () => page.request.get(`${url}/api/snapshot`).then(response => response.json());
    expect((await snapshot()).body_workbench.body_id).toBe(bodyId);
    await expect(page.locator("#body-summary")).toHaveText(/^Body: /);
    await page.locator("#body-summary").click();
    await page.getByRole("button", { name: "Join this Body", exact: true }).click();
    await expect(page.locator("#body-membership-status")).toHaveText("Browser membership: admitted", { timeout: 10_000 });
    await expect.poll(
      async () => (await snapshot()).body_host_offer_evidence?.stage,
      { message: `Body Host offer evidence was not adopted: ${probe.output()}`, timeout: 10_000 },
    ).toBe("AdmittedMembership");
    await page.getByRole("button", { name: "Request active Form evidence", exact: true }).click();
    await expect(page.locator("#body-capability-evidence-status")).toContainText("SelfReported evidence");
    await page.getByRole("button", { name: "Plan active Forms on this Host", exact: true }).click();
    await expect(page.locator("#body-capability-evidence-status")).toContainText("Body replanned");
    const proposal = await page.request.get(`${url}/api/body-execution-proposal`).then(response => response.json());
    await page.getByRole("button", { name: "Start proposed Body Play", exact: true }).click();
    await expect(page.locator("#body-execution-status")).toContainText("Body Play running");
    await expect(page.locator("#body-execution-output")).toContainText("3");
    await page.getByRole("group", { name: "Body Play input", exact: true }).hover();
    await page.mouse.down();
    await expect(page.locator('[data-presentation-kind="presentation/indicator-state"]')).toHaveText("true");
    await page.mouse.up();
    await expect(page.locator('[data-presentation-kind="presentation/indicator-state"]')).toHaveText("false");
    await expect(page.locator("#body-execution-status")).toContainText("Body Play running", { timeout: 10_000 });
    const running = await snapshot();
    expect(running.body_planning.execution_claims[0].phase).toBe("Started");
    await page.getByRole("button", { name: "Cancel Body Play", exact: true }).click();
    await expect(page.locator("#body-execution-status")).toContainText("Body Play cancelled", { timeout: 10_000 });
    const terminal = await snapshot();
    expect(terminal.body_planning.body_id).toBe(bodyId);
    expect(terminal.body_planning.execution_claims).toHaveLength(1);
    expect(terminal.body_planning.execution_claims[0].phase.Terminal.disposition).toBe("cancelled");
    const execution = JSON.parse(await page.locator("#body-execution-evidence").textContent());
    expect(execution.play.body_id).toBe(bodyId);
    expect(execution.receipt.active_play_id).toBe(terminal.body_planning.execution_claims[0].play.active_play_id);
    expect(execution.receipt.timer_completions).toBe(4);
    expect(execution.receipt.manifestation_completions).toBe(6);
    const signs = execution.receipt.kernel_signs;
    expect(signs.schema).toBe("conduit.browser/kernel-sign-evidence@1");
    expect(signs.active_play_id).toBe(execution.play.active_play_id);
    expect(signs.host_id).toBe(terminal.body_planning.execution_claims[0].host_id);
    expect(signs.boot_id).toBe(terminal.body_planning.execution_claims[0].boot_id);
    expect(signs.events.length).toBeGreaterThan(0);
    expect(signs.events.length).toBeLessThanOrEqual(signs.item_capacity);
    expect(signs.events.some(event => event.kind === "HostOperationCompleted")).toBe(true);
    for (const completion of signs.events.filter(event => event.kind === "HostOperationCompleted")) {
      expect(signs.events.some(request => request.kind === "HostOperationRequested"
        && request.node === completion.node && request.request === completion.request
        && request.sequence < completion.sequence)).toBe(true);
    }
    expect(signs.placements.map(binding => binding.placement_id).sort()).toEqual(
      proposal.plan.forms.flatMap(form => form.plan.fragments.flatMap(fragment => fragment.placements.map(placement => placement.placement_id))).sort(),
    );
    await page.getByText("Inspect selected Body Plan", { exact: true }).click();
    const inspection = page.locator("#body-plan-inspection");
    await expect(inspection).toContainText(proposal.plan.plan_id);
    await expect(inspection).toContainText("not current availability or physical proof");
    await expect(inspection).not.toContainText("undefined");
    for (const form of proposal.plan.forms) {
      await inspection.getByText(`Form ${form.plan.checked_form_id}`, { exact: true }).click();
      for (const fragment of form.plan.fragments) {
        for (const placement of fragment.placements) {
          const gear = inspection.locator(`[data-placement-id="${placement.placement_id}"]`);
          await gear.locator("summary").click();
          for (const identity of [placement.gear_id, placement.kind_id, placement.capability_id,
            placement.host_id, placement.boot_id, placement.implementation_id, placement.artifact_id]) {
            await expect(gear).toContainText(identity);
          }
          await expect(gear).toContainText(JSON.stringify(placement.resources));
          await expect(gear).toContainText(JSON.stringify(placement.host_operations));
        }
      }
    }
    const retained = await page.request.get(`${url}/api/body-evidence`).then(response => response.json());
    expect((await snapshot()).body_planning).toEqual(terminal.body_planning);
    expect(retained.body_id).toBe(bodyId);
    expect(retained.records.slice(0, born.records.length)).toEqual(born.records);
    expect(retained.body.workset).toEqual(born.body.workset);
    expect(retained.wakes).toHaveLength(1);
    expect(errors).toEqual([]);
  } finally {
    for (const process of processes) process.kill();
    await rm(temporary, { recursive: true, force: true });
  }
});

test("Rosehip House runs reusable bounded measurement processing and history in one Body", async ({ page }) => {
  const temporary = await mkdtemp(join(tmpdir(), "conduit-rosehip-measurement-"));
  const processes = [];
  try {
    const { current, evidencePath } = await birthWorkspaceEvidence(
      page,
      temporary,
      "Rosehip House",
      ["Button Across the Room", "Little Seismograph"],
      "rosehip-measurement.json",
    );
    const bodyId = current.body_id;
    expect(bodyId).toMatch(/^[0-9a-f]{64}$/);
    const born = JSON.parse(await readFile(evidencePath, "utf8"));
    expect(born.body_id).toBe(bodyId);
    expect(born.body.workset.forms).toHaveLength(2);

    const probe = await startPresenceProbe(["--body-evidence", evidencePath]);
    processes.push(probe.process);
    const patchbay = spawn("target/debug/patchbay-html", [
      "--body-evidence", evidencePath, "--external-reader", "--body-invitation", probe.url,
      "--form", "button-across-room", "forms/button-across-room/main.conduit",
      "--form", "little-seismograph-display", "forms/little-seismograph/main.conduit",
    ], { cwd: new URL("../..", import.meta.url).pathname, stdio: ["ignore", "pipe", "pipe"] });
    processes.push(patchbay);
    const url = await new Promise((resolve, reject) => {
      let output = "";
      const timeout = setTimeout(() => reject(new Error(`Rosehip Patchbay start timed out: ${output}`)), 10_000);
      const inspect = chunk => {
        output += chunk;
        const match = output.match(/PATCHBAY_HTML_URL=(http:\/\/127\.0\.0\.1:\d+)/);
        if (match) { clearTimeout(timeout); resolve(match[1]); }
      };
      patchbay.stdout.on("data", inspect);
      patchbay.stderr.on("data", inspect);
      patchbay.once("exit", code => { clearTimeout(timeout); reject(new Error(`Rosehip Patchbay exited ${code}: ${output}`)); });
    });
    await page.goto(url);
    await expect(page.locator("#body-summary")).toHaveText(/^Body: /);
    await page.locator("#body-summary").click();
    await page.getByRole("button", { name: "Join this Body", exact: true }).click();
    await expect(page.locator("#body-membership-status")).toHaveText("Browser membership: admitted", { timeout: 10_000 });
    const snapshot = () => page.request.get(`${url}/api/snapshot`).then(response => response.json());
    await expect.poll(async () => (await snapshot()).body_host_offer_evidence?.stage, { timeout: 10_000 }).toBe("AdmittedMembership");
    await page.getByRole("button", { name: "Request active Form evidence", exact: true }).click();
    await expect(page.locator("#body-capability-evidence-status")).toContainText("SelfReported evidence");
    await page.getByRole("button", { name: "Plan active Forms on this Host", exact: true }).click();
    await expect(page.locator("#body-capability-evidence-status")).toContainText("Body replanned");
    const proposal = await page.request.get(`${url}/api/body-execution-proposal`).then(response => response.json());
    expect(proposal.plan.body_id).toBe(bodyId);
    expect(proposal.plan.forms).toHaveLength(2);
    await page.getByRole("button", { name: "Start proposed Body Play", exact: true }).click();
    await expect(page.locator("#body-execution-status")).toContainText("Body Play running", { timeout: 10_000 });
    await expect(page.locator('[data-presentation-kind="presentation/measurement-plot"]')).toHaveText("plot 1 samples · 0 omitted");
    await expect(page.locator('[data-presentation-kind="presentation/measurement-threshold"]')).toHaveText("threshold Above · Some(RoseAbove)");
    await page.getByRole("button", { name: "Cancel Body Play", exact: true }).click();
    await expect(page.locator("#body-execution-status")).toContainText("Body Play cancelled", { timeout: 10_000 });
    expect((await snapshot()).body_planning.body_id).toBe(bodyId);
  } finally {
    for (const process of processes) if (process.exitCode === null) process.kill("SIGTERM");
    await rm(temporary, { recursive: true });
  }
});

test("Rosehip House is born once in Workspace and later admits independent browser Hosts", async ({ page, browser }) => {
  const temporary = await mkdtemp(join(tmpdir(), "conduit-rosehip-house-"));
  const processes = [];
  const contexts = [];
  try {
    const { current, evidencePath } = await birthWorkspaceEvidence(
      page,
      temporary,
      "Rosehip House",
      ["Button Across the Room"],
      "rosehip-house.json",
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
      const inspect = (chunk) => {
        output += chunk.toString();
        const body = output.match(/body_url=([^\s]+)/)?.[1];
        const spawnHex = output.match(/spawn_hex=([^\s]+)/)?.[1];
        if (body && spawnHex) { clearTimeout(timeout); resolve({ body, spawnHex }); }
      };
      capstone.stdout.on("data", inspect);
      capstone.stderr.on("data", inspect);
      capstone.once("exit", (code) => { clearTimeout(timeout); if (code !== 0) reject(new Error(`Rosehip admission exited (${code})\n${output}`)); });
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
    const receipt = output.split("\n").filter((line) => line.startsWith("{")).map((line) => JSON.parse(line)).at(-1);
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
    await rm(temporary, { recursive: true });
  }
});
