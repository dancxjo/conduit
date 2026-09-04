import { spawn } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { startPresenceProbe } from "./browser-presence-support.mjs";

let server;
let hostedTruth;
let membershipProbe;
let temporaryEvidenceDirectory;

async function openPatchbay(page, patchbayArguments) {
  server = spawn("target/debug/patchbay-html", patchbayArguments, {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Body workbench was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/PATCHBAY_HTML_URL=(http:\/\/127\.0\.0\.1:\d+)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    server.stdout.on("data", inspect);
    server.stderr.on("data", inspect);
    server.once("exit", code => reject(new Error(`Body workbench exited (${code})\n${output}`)));
  });
  await page.goto(url);
  await expect(page.locator('[data-application-key="product-status"]')).toContainText("Presentation revision");
  return url;
}

async function openWorkbench(page, entrance, extraArguments = []) {
  return openPatchbay(page, ["--body-workbench-fixture", entrance, ...extraArguments]);
}

async function writeFixtureEvidence(page) {
  const fixtureUrl = await openWorkbench(page, "external");
  const snapshot = await page.request.get(new URL("/api/snapshot", fixtureUrl).href).then(response => response.json());
  temporaryEvidenceDirectory = await mkdtemp(join(tmpdir(), "conduit-patchbay-membership-"));
  const evidencePath = join(temporaryEvidenceDirectory, "body.json");
  await writeFile(evidencePath, Buffer.from(snapshot.body_workbench.encoded_evidence));
  server.kill();
  return evidencePath;
}

test("an invited browser remains outside the Body until explicit join", async ({ page }) => {
  const evidencePath = await writeFixtureEvidence(page);
  membershipProbe = await startPresenceProbe(["--body-evidence", evidencePath]);
  const patchbayUrl = await openPatchbay(page, ["--body-evidence", evidencePath, "--external-reader", "--body-invitation", membershipProbe.url]);
  const bodyId = await page.request.get(new URL("/api/snapshot", patchbayUrl).href).then(async response => (await response.json()).body_workbench.body_id);
  await expect(page.getByRole("button", { name: "Join this Body", exact: true })).toBeVisible();
  await expect(page.locator("#body-membership-status")).toContainText("not a member");
  expect(await page.evaluate(() => globalThis.__patchbayMembership)).toBeUndefined();
  expect(membershipProbe.output()).not.toContain("admitted");
  await page.getByRole("button", { name: "Join this Body", exact: true }).click();
  await expect.poll(() => page.evaluate(() => globalThis.__patchbayMembership?.state())).toBe("admitted");
  await expect(page.locator("#body-membership-status")).toContainText("admitted");
  const admittedIdentity = await page.evaluate(() => ({
    hostId: globalThis.__patchbayMembership.hostId,
    bootId: globalThis.__patchbayMembership.bootId,
  }));
  const membershipCredential = await page.evaluate(() => {
    const credential = globalThis.__patchbayMembership.membershipCredential();
    return { credential, frozen: Object.isFrozen(credential) };
  });
  expect(membershipCredential.frozen).toBe(true);
  expect(membershipCredential.credential.body_id).toBe(bodyId);
  expect(membershipCredential.credential.host_id).toBe(admittedIdentity.hostId);
  expect(membershipCredential.credential.boot_id).toBe(admittedIdentity.bootId);
  await expect.poll(() => page.evaluate(() => globalThis.__patchbayMembership.biographyEvidence()?.body_id)).toBe(bodyId);
  const admissionBiography = await page.evaluate(() => {
    const evidence = globalThis.__patchbayMembership.biographyEvidence();
    const part = evidence.membership.parts.find(candidate => candidate.part_id === globalThis.__patchbayMembership.membershipCredential().part_id);
    return {
      frozen: Object.isFrozen(evidence) && Object.isFrozen(evidence.membership) && Object.isFrozen(evidence.records),
      bodyId: evidence.body_id,
      part,
      recordCount: evidence.records.length,
    };
  });
  expect(admissionBiography.frozen).toBe(true);
  expect(admissionBiography.bodyId).toBe(bodyId);
  expect(admissionBiography.part.current.host_id).toBe(admittedIdentity.hostId);
  expect(admissionBiography.part.current.boot_id).toBe(admittedIdentity.bootId);
  expect(admissionBiography.recordCount).toBeGreaterThan(1);
  expect(admittedIdentity.hostId).toMatch(/^browser\//);
  expect(admittedIdentity.bootId).toMatch(/^browser-boot\//);
  await expect(page.locator('[data-application-slot="body-membership-facts"]')).toContainText(admittedIdentity.hostId);
  await expect(page.locator('[data-application-slot="body-membership-facts"]')).toContainText(admittedIdentity.bootId);
  await expect(page.locator('[data-application-slot="body-membership-facts"]')).toContainText(bodyId);
  await expect(page.locator('[data-application-slot="body-membership-facts"]')).toContainText(membershipCredential.credential.part_id);
  await expect(page.locator('[data-application-slot="body-membership-facts"]')).toContainText(membershipCredential.credential.credential_id);
  await expect(page.locator('[data-application-slot="body-membership-facts"]')).toContainText("Membershipadmitted");
  await expect(page.locator('[data-application-slot="body-membership-facts"]')).toContainText("Presenceavailable");
  const advertisement = await page.evaluate(() => globalThis.__patchbayMembership.advertisement);
  const offerFacts = page.locator('[data-application-slot="body-membership-offers"]');
  await expect(offerFacts).toContainText("Offer availabilityAvailable from current presence");
  await expect(offerFacts).toContainText(advertisement.profile);
  await expect(offerFacts).toContainText(`Offer generation${advertisement.offer_generation}`);
  for (const offer of advertisement.capabilities) await expect(offerFacts).toContainText(offer.capability_id);
  for (const offer of advertisement.planner_capabilities) await expect(offerFacts).toContainText(offer.profile_id);
  for (const offer of advertisement.resources) await expect(offerFacts).toContainText(offer.pool_id);
  const inspectedOffer = advertisement.capabilities.at(-1);
  const capabilitySelect = page.getByLabel("Inspect capability", { exact: true });
  await capabilitySelect.selectOption(inspectedOffer.capability_id);
  const capabilityFacts = page.locator('[data-application-slot="body-capability-facts"]');
  await expect(capabilityFacts).toContainText(inspectedOffer.capability_id);
  await expect(capabilityFacts).toContainText(inspectedOffer.kind_id);
  await expect(capabilityFacts).toContainText(inspectedOffer.implementation_id);
  await expect(capabilityFacts).toContainText(inspectedOffer.artifact_id);
  await expect(capabilityFacts).toContainText(inspectedOffer.execution_profile_id);
  await expect(capabilityFacts).toContainText(`active=${inspectedOffer.limits.max_active_instances}`);
  await expect(capabilityFacts).toContainText(`queue-items=${inspectedOffer.limits.max_queue_items}`);
  await expect(capabilityFacts).toContainText(`queue-bytes=${inspectedOffer.limits.max_queue_bytes}`);
  for (const port of [...inspectedOffer.inputs, ...inspectedOffer.outputs]) {
    await expect(capabilityFacts).toContainText(port.port_id);
    await expect(capabilityFacts).toContainText(port.value_kind);
  }
  const inspectedResource = advertisement.resources.at(-1);
  await page.getByLabel("Inspect resource pool", { exact: true }).selectOption(inspectedResource.pool_id);
  const resourceFacts = page.locator('[data-application-slot="body-resource-facts"]');
  await expect(resourceFacts).toContainText(inspectedResource.pool_id);
  await expect(resourceFacts).toContainText(inspectedResource.class_id);
  await expect(resourceFacts).toContainText(`Advertised capacity${inspectedResource.capacity_units} units`);
  await expect(resourceFacts).toContainText("Reservationnone · advertisement only");
  await expect(resourceFacts).toContainText("Current utilizationnot reported by this advertisement");
  const inspectedPlanner = advertisement.planner_capabilities.at(-1);
  await page.getByLabel("Inspect planner profile", { exact: true }).selectOption(inspectedPlanner.profile_id);
  const plannerFacts = page.locator('[data-application-slot="body-planner-facts"]');
  await expect(plannerFacts).toContainText(inspectedPlanner.profile_id);
  await expect(plannerFacts).toContainText(`Host advertisements${inspectedPlanner.limits.maximum_host_advertisements}`);
  await expect(plannerFacts).toContainText(`Gears${inspectedPlanner.limits.maximum_gears}`);
  await expect(plannerFacts).toContainText(`Connections${inspectedPlanner.limits.maximum_connections}`);
  await expect(plannerFacts).toContainText("Planner selectionnone · advertisement only");
  await page.getByRole("button", { name: "Disconnect this browser Host", exact: true }).click();
  await expect.poll(() => page.evaluate(() => globalThis.__patchbayMembership?.state())).toBe("offline");
  await expect(page.locator("#body-membership-status")).toHaveText("Browser presence disconnected. Durable Body membership was not revoked.");
  await expect(page.locator('[data-application-slot="body-membership-facts"]')).toContainText("Membershipoffline");
  await expect(page.locator('[data-application-slot="body-membership-facts"]')).toContainText("Presenceunavailable");
  await expect(offerFacts).toContainText("Offer availabilityUnavailable · retained advertisement only");
  expect(await page.evaluate(() => ({
    hostId: globalThis.__patchbayMembership.hostId,
    bootId: globalThis.__patchbayMembership.bootId,
  }))).toEqual(admittedIdentity);
  await expect(page.getByRole("button", { name: "Join this Body", exact: true })).toBeEnabled();
  await expect.poll(membershipProbe.output).toContain("unavailable reason=session-lost");
});

test("an invitation for a different Body refuses before admission", async ({ page }) => {
  const evidencePath = await writeFixtureEvidence(page);
  membershipProbe = await startPresenceProbe();
  await openPatchbay(page, ["--body-evidence", evidencePath, "--external-reader", "--body-invitation", membershipProbe.url]);
  await page.getByRole("button", { name: "Join this Body", exact: true }).click();
  await expect.poll(() => page.evaluate(() => globalThis.__patchbayMembership?.state())).toBe("refused:wrong-body");
  await expect(page.locator("#body-membership-status")).toHaveText("Invitation refused: it belongs to a different Body than the evidence open here.");
  await expect(page.getByRole("button", { name: "Join this Body", exact: true })).toBeDisabled();
  expect(membershipProbe.output()).not.toContain("admitted");
});

test("an admitted browser visibly returns once with the same Host and Boot", async ({ page }) => {
  const evidencePath = await writeFixtureEvidence(page);
  membershipProbe = await startPresenceProbe(["--body-evidence", evidencePath, "--reconnect"]);
  await openPatchbay(page, ["--body-evidence", evidencePath, "--external-reader", "--body-invitation", membershipProbe.url]);
  await page.getByRole("button", { name: "Join this Body", exact: true }).click();
  await expect.poll(() => page.evaluate(() => globalThis.__patchbayMembership?.state())).toBe("admitted");
  const identity = await page.evaluate(() => ({
    hostId: globalThis.__patchbayMembership.hostId,
    bootId: globalThis.__patchbayMembership.bootId,
  }));
  await expect.poll(membershipProbe.output).toContain("unavailable reason=session-lost-for-return");
  await expect.poll(membershipProbe.output).toContain("returned-renewed sequence=4");
  await expect(page.locator("#body-membership-status")).toHaveText("Browser presence returned for the same current Boot.");
  await expect.poll(() => page.evaluate(() => globalThis.__patchbayMembership.presenceState())).toBe("available");
  expect(await page.evaluate(() => ({
    hostId: globalThis.__patchbayMembership.hostId,
    bootId: globalThis.__patchbayMembership.bootId,
  }))).toEqual(identity);
  await page.getByRole("button", { name: "Disconnect this browser Host", exact: true }).click();
});

test.afterEach(async () => {
  server?.kill();
  membershipProbe?.process.kill();
  membershipProbe = null;
  if (temporaryEvidenceDirectory) await rm(temporaryEvidenceDirectory, { recursive: true, force: true });
  temporaryEvidenceDirectory = null;
});

for (const entrance of ["hosted", "external"]) {
  test(`${entrance} graduated Body opens in the same semantic workbench`, async ({ page }) => {
    if (entrance === "external") await page.setViewportSize({ width: 390, height: 844 });
    await openWorkbench(page, entrance);
    const snapshot = await page.request.get(new URL("/api/snapshot", page.url()).href).then(response => response.json());
    const originalEvidence = snapshot.body_workbench.encoded_evidence;
    const attachment = snapshot.body_workbench.entrance;
    if (entrance === "hosted") {
      expect(attachment).toEqual({
        kind: "hosted",
        plan_id: "plan/roseau-hosted-patchbay",
        implementation_id: "browser/patchbay-surface@1",
      });
    } else {
      expect(attachment).toEqual({ kind: "external-reader" });
    }

    const current = snapshot.body_workbench.current;
    const history = snapshot.body_workbench.history;
    const projectedTruth = {
      body_id: snapshot.body_workbench.body_id,
      active_forms: current.active_forms,
      workload_revision: current.workload_revision,
      lifecycle: current.lifecycle,
      current_hosts: current.current_hosts,
      biography_identity: history.entries.map(entry => ({
        moment: entry.moment,
        sign_id: entry.exact.record.sign_id,
      })),
    };
    if (entrance === "hosted") {
      hostedTruth = projectedTruth;
    } else {
      expect(projectedTruth).toEqual(hostedTruth);
    }
    expect(JSON.stringify(history)).not.toMatch(/timestamp|wall.?clock|utc/i);
    expect(history.entries.every(entry => entry.linear.includes(snapshot.body_workbench.body_id))).toBe(true);

    await expect(page.getByRole("heading", { name: "Roseau", exact: true })).toBeVisible();
    await expect(page.locator("#body-workbench-status")).toContainText("Lulled · workload revision 0 · 1 Part · 1 current Host");
    await expect(page.locator("#body-evidence-status")).toHaveText("Evidence revision 1 matches the saved biography.");
    await expect(page.locator("#body-workbench-placement")).toContainText(
      entrance === "hosted" ? "hosted by this Body" : "external Patchbay",
    );
    const activeForms = page.locator('#body-workbench-forms [data-application-component="artifact"]');
    await expect(activeForms).toHaveCount(2);
    await expect(activeForms.filter({ hasText: "checked/roseau-program" })).toHaveCount(1);
    await expect(activeForms.filter({ hasText: "checked/roseau-recorder" })).toHaveCount(1);
    await expect(page.locator("#body-workbench-facts")).toContainText("Workload revision");
    await expect(page.locator("#body-workbench-facts")).toContainText("0");
    const availableForms = page.locator('#body-workbench-available [data-application-component="artifact"]');
    await expect(availableForms).toHaveCount(3);
    await expect(availableForms.filter({ hasText: "Hello" })).toHaveCount(1);
    const reviewedSearch = page.getByRole("searchbox", { name: "Search reviewed Forms" });
    await reviewedSearch.fill("greet");
    await expect(availableForms).toHaveCount(1);
    await expect(availableForms.filter({ hasText: "Greet" })).toHaveCount(1);
    await expect(page.locator("#body-form-results-status")).toHaveText("1 of 3 reviewed Forms match");
    await reviewedSearch.fill("");
    await expect(availableForms).toHaveCount(3);
    await expect(availableForms.filter({ hasText: "Hello" }).getByRole("button", { name: "Add to Body", exact: true })).toBeEnabled();
    if (entrance === "external") {
      const bounds = await page.locator("#body-workbench").boundingBox();
      expect(bounds.x).toBeGreaterThanOrEqual(0);
      expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
    }
    await expect(page.getByRole("button", { name: "Wake", exact: true })).toBeVisible();
    const workbenchNavigation = page.getByRole("navigation", { name: "Body workbench" });
    await expect(workbenchNavigation.getByRole("button"))
      .toHaveText(["Program", "Body", "History"]);

    const historyButton = workbenchNavigation.getByRole("button", { name: "History", exact: true });
    if (entrance === "external") {
      await historyButton.focus();
      await historyButton.press("Enter");
    } else {
      await historyButton.click();
    }
    await expect(page.getByRole("heading", { name: "What has happened to it?" })).toBeVisible();
    await expect(page.locator('#body-history [data-application-component="artifact"]')).toHaveCount(4);
    await expect(page.locator("#body-history")).toContainText("Graduated from the Crèche");
    await page.getByText("Linear BODY / SIGNS evidence", { exact: true }).click();
    await expect(page.locator('#body-linear [data-application-component="artifact"]')).toHaveCount(4);
    await expect(page.locator("body")).toHaveAttribute("data-place", "Body");
    await expect(page.locator("body")).toHaveAttribute("data-aspect", "Signs");

    await workbenchNavigation.getByRole("button", { name: "Program", exact: true }).click();
    await expect(page.locator("body")).toHaveAttribute("data-place", "Program");
    await expect(page.locator("body")).toHaveAttribute("data-aspect", "Structure");

    await workbenchNavigation.getByRole("button", { name: "Body", exact: true }).click();
    await page.getByText("Exact Body and workload identity", { exact: true }).click();
    await expect(page.locator("#body-workbench-exact")).toContainText(snapshot.body_workbench.body_id);

    await page.getByRole("button", { name: "Wake", exact: true }).click();
    await expect(page.locator("#front-door-feedback")).toContainText(
      "wake Refused(OperationUnavailable)",
    );
    const afterAction = await page.request.get(new URL("/api/snapshot", page.url()).href).then(response => response.json());
    expect(afterAction.body_workbench.encoded_evidence).toEqual(originalEvidence);

    if (entrance === "external") {
      const recorder = activeForms.filter({ hasText: "checked/roseau-recorder" });
      await recorder.getByRole("button", { name: "Remove from Body", exact: true }).click();
      await expect(activeForms).toHaveCount(1);
      await expect(page.locator("#body-workbench-status")).toContainText("workload revision 1");
      await expect(page.locator("#body-evidence-status")).toHaveText("Evidence revision 2 has unsaved workload changes.");
      expect(await page.evaluate(() => {
        const event = new Event("beforeunload", { cancelable: true });
        window.dispatchEvent(event);
        return event.defaultPrevented;
      })).toBe(true);
      await expect(activeForms.getByRole("button", { name: "Remove from Body", exact: true })).toBeEnabled();
      await expect(page.locator('#body-history [data-application-component="artifact"]')).toHaveCount(5);
      const changed = await page.request.get(new URL("/api/snapshot", page.url()).href).then(response => response.json());
      expect(changed.body_workbench.body_id).toBe(snapshot.body_workbench.body_id);
      expect(changed.body_workbench.current.active_forms).toHaveLength(1);
      expect(changed.body_workbench.current.workload_revision).toBe(1);
      expect(changed.interaction.last_disposition).toBe("Succeeded");

      await activeForms.getByRole("button", { name: "Remove from Body", exact: true }).click();
      await expect(activeForms).toHaveCount(0);
      await expect(page.locator("#body-workbench-facts")).toContainText("Active Forms");
      await expect(page.locator("#body-workbench-facts")).toContainText("0");
      await expect(page.locator("#body-workbench-status")).toContainText("workload revision 2");
      await expect(workbenchNavigation.getByRole("button")).toHaveText(["Body", "History"]);
      await expect(page.locator('#body-history [data-application-component="artifact"]')).toHaveCount(6);
      const idle = await page.request.get(new URL("/api/snapshot", page.url()).href).then(response => response.json());
      expect(idle.body_workbench.body_id).toBe(snapshot.body_workbench.body_id);
      expect(idle.body_workbench.current.active_forms).toHaveLength(0);
      expect(idle.body_workbench.current.workload_revision).toBe(2);
      expect(idle.navigation.cursor.place).toBe("Body");

      await availableForms.filter({ hasText: "Hello" }).getByRole("button", { name: "Add to Body", exact: true }).click();
      await expect(availableForms).toHaveCount(2);
      await expect(activeForms).toHaveCount(1);
      await expect(activeForms.filter({ hasText: "Hello" })).toHaveCount(1);
      await expect(page.locator("#body-workbench-status")).toContainText("workload revision 3");
      await expect(page.locator('#body-history [data-application-component="artifact"]')).toHaveCount(7);
      const added = await page.request.get(new URL("/api/snapshot", page.url()).href).then(response => response.json());
      expect(added.body_workbench.body_id).toBe(snapshot.body_workbench.body_id);
      expect(added.body_workbench.current.active_forms).toHaveLength(1);
      expect(added.body_workbench.current.active_forms.some(form => form.checked_form_id === added.body_workbench.reviewed_forms[0].checked_form_id)).toBe(true);
      expect(added.body_workbench.current.workload_revision).toBe(3);
      const downloadPromise = page.waitForEvent("download");
      await page.getByRole("button", { name: "Save Body evidence", exact: true }).click();
      const download = await downloadPromise;
      expect(download.suggestedFilename()).toBe(`conduit-body-${snapshot.body_workbench.body_id}.json`);
      const chunks = [];
      for await (const chunk of await download.createReadStream()) chunks.push(chunk);
      const exported = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      expect(exported.body_id).toBe(snapshot.body_workbench.body_id);
      expect(exported.body.workload_revision).toBe(3);
      expect(exported.body.workset.forms).toHaveLength(1);
      await expect(page.locator("#body-evidence-status")).toHaveText("Evidence revision 4 matches the saved biography.");
      expect(await page.evaluate(() => {
        const event = new Event("beforeunload", { cancelable: true });
        window.dispatchEvent(event);
        return event.defaultPrevented;
      })).toBe(false);
    }

    server.kill();
    await expect(page.getByRole("heading", { name: "Roseau", exact: true })).toBeVisible();
    expect(snapshot.body_workbench.encoded_evidence).toEqual(originalEvidence);
  });
}
