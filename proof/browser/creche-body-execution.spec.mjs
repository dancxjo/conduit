import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";
import { startPresenceProbe } from "./browser-presence-support.mjs";
import { selectBirthForm } from "./creche-test-actions.mjs";

test("a Crèche-born canonical workset continues as the same executing Body", async ({ page }) => {
  const temporary = await mkdtemp(join(tmpdir(), "conduit-creche-execution-"));
  const processes = [];
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  try {
    const creche = await startStaticProduct("target/creche-product", "/conduit/creche/");
    processes.push(creche.child);
    await page.goto(creche.url);
    await expect(page.locator("#host-state")).toHaveText("Crèche ready");
    const birth = page.locator(".body-birth-runner");
    for (const title of ["Button Across Room", "Clock-demo", "Desk Telegraph"]) {
      await selectBirthForm(birth, title);
    }
    await birth.getByRole("button", { name: "Review workload", exact: true }).click();
    await expect(birth.getByRole("button", { name: "Birth Body", exact: true })).toBeEnabled();
    await birth.getByRole("button", { name: "Birth Body", exact: true }).click();
    await expect(birth).toHaveAttribute("data-body-id", /.+/);
    const bodyId = await birth.getAttribute("data-body-id");
    const birthSignId = await birth.getAttribute("data-birth-sign-id");
    await page.getByRole("button", { name: "2. First Host", exact: true }).click();
    await page.getByRole("button", { name: "Give this Body its first Host", exact: true }).click();
    await page.getByRole("button", { name: "4. Graduate", exact: true }).click();
    await page.getByRole("button", { name: "Finish without hosted Patchbay", exact: true }).click();
    await page.getByRole("button", { name: "End the Crèche", exact: true }).click();
    const downloading = page.waitForEvent("download");
    await page.getByRole("button", { name: "Save Body evidence", exact: true }).click();
    const download = await downloading;
    expect(download.suggestedFilename()).toBe(`conduit-body-${bodyId}.json`);
    const evidencePath = join(temporary, "body.json");
    await download.saveAs(evidencePath);
    const born = JSON.parse(await readFile(evidencePath, "utf8"));
    expect(born.body_id).toBe(bodyId);
    expect(born.records[0].sign_id).toBe(birthSignId);
    expect(born.body.workset.forms).toHaveLength(3);
    expect(born.body.workload_revision).toBe(0);

    const probe = await startPresenceProbe(["--body-evidence", evidencePath]);
    processes.push(probe.process);
    const patchbay = spawn("target/debug/patchbay-html", [
      "--body-evidence", evidencePath, "--external-reader", "--body-invitation", probe.url,
      ...["button-across-room", "clock", "desk-telegraph"].flatMap(name => ["--form", name, `forms/${name}/main.conduit`]),
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
    await page.getByRole("button", { name: "Join this Body", exact: true }).click();
    await expect.poll(async () => (await snapshot()).body_host_offer_evidence?.stage).toBe("AdmittedMembership");
    await page.getByRole("button", { name: "Request active Form evidence", exact: true }).click();
    await expect(page.locator("#body-capability-evidence-status")).toContainText("SelfReported evidence");
    await page.getByRole("button", { name: "Plan active Forms on this Host", exact: true }).click();
    await expect(page.locator("#body-capability-evidence-status")).toContainText("Body replanned");
    const proposal = await page.request.get(`${url}/api/body-execution-proposal`).then(response => response.json());
    await page.getByRole("button", { name: "Start proposed Body Play", exact: true }).click();
    await expect(page.locator("#body-execution-status")).toContainText("Body Play running");
    await expect(page.locator("#body-execution-output")).toContainText("CALLING");
    await page.getByRole("group", { name: "Body Play input", exact: true }).hover();
    await page.mouse.down();
    await expect(page.locator('[data-presentation-kind="presentation/indicator-state"]')).toHaveText("true");
    await page.mouse.up();
    await expect(page.locator('[data-presentation-kind="presentation/indicator-state"]')).toHaveText("false");
    await expect(page.locator("#body-execution-status")).toContainText("Body Play completed", { timeout: 10_000 });
    const terminal = await snapshot();
    expect(terminal.body_planning.body_id).toBe(bodyId);
    expect(terminal.body_planning.execution_claims).toHaveLength(1);
    expect(terminal.body_planning.execution_claims[0].phase.Terminal.disposition).toBe("completed");
    const execution = JSON.parse(await page.locator("#body-execution-evidence").textContent());
    expect(execution.play.body_id).toBe(bodyId);
    expect(execution.receipt.active_play_id).toBe(terminal.body_planning.execution_claims[0].play.active_play_id);
    expect(execution.receipt.timer_completions).toBe(4);
    expect(execution.receipt.manifestation_completions).toBe(7);
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
