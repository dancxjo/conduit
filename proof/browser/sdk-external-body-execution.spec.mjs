import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { startPresenceProbe } from "./browser-presence-support.mjs";
import { prepareFieldStationPackage } from "./field-station-package.mjs";
import { birthWorkspaceEvidence } from "./workspace-body-evidence-support.mjs";

test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(120_000);
  await prepareFieldStationPackage();
});

test("a Workspace Body continues through the browser SDK without a Patchbay page runtime", async ({ page }) => {
  const journey = await openExternalBody(page, {
    friendlyName: "SDK Workbench",
    titles: ["Button Across the Room", "Clock", "Desk Telegraph"],
    plots: [
      ["button-across-room", "plots/button-across-room/main.conduit"],
      ["clock", "plots/clock/main.conduit"],
      ["desk_telegraph", "plots/desk-telegraph/main.conduit"],
    ],
  });
  try {
    await page.evaluate(() => globalThis.__conduitSdkParticipation.begin());
    await expect(page.locator("#external-body-output output")).toHaveCount(3);
    await expect(page.locator("#external-body-output")).toContainText("3");
    await page.getByRole("group", { name: "External Body Play input", exact: true }).hover();
    await page.mouse.down();
    await expect(page.locator('[data-presentation-kind="presentation/indicator-state"]')).toHaveText("true");
    await page.mouse.up();
    await expect(page.locator('[data-presentation-kind="presentation/indicator-state"]')).toHaveText("false");
    await expect(page.locator('[data-presentation-kind="presentation/tick"]'))
      .toHaveText(/^\d+$/, { timeout: 15_000 });
    const { receipt, terminal } = await finishExternalBody(page, journey);
    expect(receipt).toMatchObject({
      schema: "conduit.tour/manifestation-receipt@3",
      active_play_id: journey.claim.play.active_play_id,
      disposition: "cancelled",
      timer_completions: 4,
      manifestation_completions: 6,
    });
    expect(terminal.body_planning.execution_claims.at(-1).phase.Terminal.disposition).toBe("cancelled");
    expect(receipt.kernel_signs).toMatchObject({
      host_id: journey.host.id,
      boot_id: journey.host.bootId,
      active_play_id: journey.claim.play.active_play_id,
    });
    expect(receipt.kernel_signs.placements.map(({ placement_id }) => placement_id).sort()).toEqual(
      journey.proposal.plan.plots.flatMap(({ plan }) => plan.fragments
        .flatMap(({ placements }) => placements.map(({ placement_id }) => placement_id))).sort(),
    );
  } finally {
    await journey.cleanup();
  }
});

test("Rosehip measurement processing and bounded history continue through the SDK", async ({ page }) => {
  const journey = await openExternalBody(page, {
    friendlyName: "Rosehip House",
    titles: ["Button Across the Room", "Little Seismograph"],
    plots: [
      ["button-across-room", "plots/button-across-room/main.conduit"],
      ["little-seismograph-display", "plots/little-seismograph/main.conduit"],
    ],
  });
  try {
    await page.evaluate(() => globalThis.__conduitSdkParticipation.begin());
    await expect(page.locator('[data-presentation-kind="presentation/measurement-plot"]'))
      .toHaveText("plot 1 samples · 0 omitted");
    await expect(page.locator('[data-presentation-kind="presentation/measurement-threshold"]'))
      .toHaveText("threshold Above · Some(RoseAbove)");
    const { receipt, terminal } = await finishExternalBody(page, journey);
    expect(receipt).toMatchObject({
      schema: "conduit.tour/manifestation-receipt@3",
      active_play_id: journey.claim.play.active_play_id,
      disposition: "cancelled",
    });
    expect(terminal.body_planning.body_id).toBe(journey.bodyId);
    expect(terminal.body_planning.execution_claims).toHaveLength(1);
  } finally {
    await journey.cleanup();
  }
});

async function openExternalBody(page, { friendlyName, titles, plots }) {
  const temporary = await mkdtemp(join(tmpdir(), "conduit-sdk-external-body-"));
  const processes = [];
  const cleanup = async () => {
    for (const process of processes) if (process.exitCode === null) process.kill("SIGTERM");
    await rm(temporary, { recursive: true, force: true });
  };
  try {
    const { current, evidencePath } = await birthWorkspaceEvidence(
      page, temporary, friendlyName, titles, "body.json",
    );
    const probe = await startPresenceProbe(["--body-evidence", evidencePath]);
    processes.push(probe.process);
    const coordinator = await startCoordinator(evidencePath, probe.url, plots);
    processes.push(coordinator.process);

    await page.goto(`/proof/browser/sdk-body-participation/?body=${encodeURIComponent(probe.url)}`);
    await expect(page.locator("#status")).toHaveText("Browser Host participating", { timeout: 15_000 });
    await expect.poll(() => page.evaluate(() => {
      const observed = globalThis.__conduitSdkParticipation?.evidence();
      return Boolean(observed?.biography && observed?.offer);
    })).toBe(true);

    const api = path => new URL(`/api/${path}`, coordinator.url).href;
    const observed = await page.evaluate(() => globalThis.__conduitSdkParticipation.evidence());
    await post(page, api("body-membership-evidence"), observed.biography);
    await post(page, api("body-host-offer-evidence"), observed.offer);
    const requirements = await get(page, api("body-planning-requirements"));
    const executable = await page.evaluate(() => globalThis.__conduitSdkParticipation.executionCapabilities());
    for (const identity of [
      "browser/exact-quantity-conversion@1", "browser/exact-temperature-difference-conversion@1",
      "browser/exact-quantity-comparison@1", "browser/exact-temperature-difference-comparison@1",
      "browser/converted-equals@1",
    ]) expect(executable).toContain(identity);
    const selection = await page.evaluate(kindIds => {
      const sdk = globalThis.__conduitSdkParticipation;
      const executable = new Set(sdk.executionCapabilities());
      const advertisement = sdk.participation.advertisement;
      const capabilities = kindIds.map(kind => advertisement.capabilities
        .find(offer => offer.kind_id === kind && executable.has(offer.capability_id)));
      if (capabilities.some(offer => !offer)) throw new Error("SDK Host cannot realize the active Body workset");
      const resourceClasses = new Set(capabilities
        .flatMap(offer => offer.resource_requirements.map(requirement => requirement.class_id)));
      return {
        capabilityIds: [...new Set(capabilities.map(offer => offer.capability_id))].sort(),
        resourcePoolIds: advertisement.resources
          .filter(offer => resourceClasses.has(offer.class_id)).map(offer => offer.pool_id).sort(),
      };
    }, requirements.kind_ids);
    await page.evaluate(requested =>
      globalThis.__conduitSdkParticipation.participation.requestOfferEvidence(requested), selection);
    await expect.poll(() => page.evaluate(() =>
      globalThis.__conduitSdkParticipation.evidence().offer?.stage)).toBe("Planning");
    const planningOffer = await page.evaluate(() => globalThis.__conduitSdkParticipation.evidence().offer);
    await post(page, api("body-host-planning-offer"), planningOffer);

    const proposal = await get(page, api("body-execution-proposal"));
    expect(proposal.plan.body_id).toBe(current.body_id);
    const observations = await page.evaluate(value =>
      globalThis.__conduitSdkParticipation.prepare(value), proposal);
    expect(observations.length).toBeGreaterThanOrEqual(2);
    const inspection = await page.evaluate(() =>
      globalThis.__conduitSdkParticipation.planInspection());
    expect(inspection).toMatchObject({
      schema: "conduit.browser/body-plan-inspection@1",
      bodyId: current.body_id,
      wakeId: proposal.wake.wake_id,
      planId: proposal.plan.plan_id,
    });
    expect(inspection.plan).toEqual(proposal.plan);
    await expect(page.locator("#body-plan-inspection")).toContainText(`Selected body Plan ${proposal.plan.plan_id}`);
    const placementCount = proposal.plan.plots.reduce((count, plot) => count
      + plot.plan.fragments.reduce((subtotal, fragment) => subtotal + fragment.placements.length, 0), 0);
    await expect(page.locator("#body-plan-inspection [data-placement-id]")).toHaveCount(placementCount);
    const host = await page.evaluate(() => globalThis.__conduitSdkParticipation.host.current());
    const claimed = await post(page, api("body-execution"), {
      schema: "conduit.body/execution-request@1",
      action: { kind: "Claim", plan_id: proposal.plan.plan_id, host_id: host.id, boot_id: host.bootId },
    });
    const claim = claimed.body_planning.execution_claims.at(-1);
    const started = await page.evaluate(identity =>
      globalThis.__conduitSdkParticipation.start(identity), claim.play);
    expect(started.play).toEqual(claim.play);
    await post(page, api("body-execution"), {
      schema: "conduit.body/execution-request@1",
      action: { kind: "Started", play: claim.play, wake_at_start: started.wakeAtStart },
    });
    return { api, bodyId: current.body_id, claim, cleanup, host, proposal };
  } catch (error) {
    await cleanup();
    throw error;
  }
}

async function finishExternalBody(page, journey) {
  const receipt = await page.evaluate(() => globalThis.__conduitSdkParticipation.cancel());
  const terminal = await post(page, journey.api("body-execution"), {
    schema: "conduit.body/execution-request@1",
    action: {
      kind: "Terminal",
      play: journey.claim.play,
      disposition: receipt.disposition,
      terminal_sign_id: receipt.terminal_sign_id,
    },
  });
  return { receipt, terminal };
}

async function startCoordinator(evidencePath, invitation, plots) {
  const process = spawn("target/debug/conduit-browser-patchbay-workbench", [
    "--body-evidence", evidencePath,
    "--external-reader",
    "--body-invitation", invitation,
    ...plots.flatMap(([name, source]) => ["--plot", name, source]),
  ], { cwd: new URL("../..", import.meta.url).pathname, stdio: ["ignore", "pipe", "pipe"] });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Body coordinator did not become ready\n${output}`)), 10_000);
    const inspect = chunk => {
      output += chunk.toString();
      const match = output.match(/PATCHBAY_HTML_URL=(http:\/\/127\.0\.0\.1:\d+)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    process.stdout.on("data", inspect);
    process.stderr.on("data", inspect);
    process.once("exit", code => { clearTimeout(timeout); reject(new Error(`Body coordinator exited (${code})\n${output}`)); });
  });
  return { process, url };
}

async function get(page, url) {
  const response = await page.request.get(url);
  if (!response.ok()) throw new Error(`${url} returned ${response.status()}: ${await response.text()}`);
  return response.json();
}

async function post(page, url, data) {
  const response = await page.request.post(url, { data });
  if (!response.ok()) throw new Error(`${url} returned ${response.status()}: ${await response.text()}`);
  return response.json();
}
