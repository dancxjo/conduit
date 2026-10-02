import { expect, test } from "@playwright/test";
import { prepareFieldStationPackage } from "./field-station-package.mjs";

test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(120_000);
  await prepareFieldStationPackage();
});

test("an external page recovers one Clock Body across a fresh Boot without resurrecting its Play", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto("/proof/browser/field-station/");
  await expect(page.locator("#status")).toHaveText("Clock Body awake", { timeout: 15_000 });

  const first = await page.evaluate(() => ({
    identities: __conduitFieldStation.identities,
    playState: __conduitFieldStation.play.state,
    recovered: __conduitFieldStation.recovered,
  }));
  expect(first.recovered).toBe(false);
  expect(first.playState).toBe("playing");
  await expect(page.locator('[data-presentation-kind="presentation/tick"]')).toHaveText("0", { timeout: 15_000 });

  await page.reload();
  await expect(page.locator("#status")).toHaveText("Clock Body awake", { timeout: 15_000 });
  const running = await page.evaluate(() => ({
    identities: __conduitFieldStation.identities,
    playState: __conduitFieldStation.play.state,
    recovered: __conduitFieldStation.recovered,
    recoverySnapshot: __conduitFieldStation.recoverySnapshot,
    patchbay: __conduitFieldStation.patchbay,
    constraintConformance: __conduitFieldStation.constraintConformance,
  }));
  expect(running.recovered).toBe(true);
  expect(running.playState).toBe("playing");
  expect(running.identities.hostId).toBe(first.identities.hostId);
  expect(running.identities.bodyId).toBe(first.identities.bodyId);
  expect(running.identities.bootId).not.toBe(first.identities.bootId);
  expect(running.identities.wakeId).not.toBe(first.identities.wakeId);
  expect(running.identities.planId).not.toBe(first.identities.planId);
  expect(running.identities.playId).not.toBe(first.identities.playId);
  expect(running.recoverySnapshot.evidence.body_id).toBe(first.identities.bodyId);
  expect(running.recoverySnapshot.realization).toBeNull();
  expect(Object.values(running.identities).every((identity) => typeof identity === "string" && identity.length > 0)).toBe(true);
  expect(new Set(Object.values(running.identities)).size).toBe(6);
  expect(running.patchbay).toMatchObject({
    schema: "conduit.browser/body-patchbay@1",
    bodyId: running.identities.bodyId,
    hostId: running.identities.hostId,
    bootId: running.identities.bootId,
    planId: running.identities.planId,
    playId: running.identities.playId,
    topology: {
      schema: "conduit.patchbay/checked-plot-projection@1",
      plot_name: "clock-demo",
    },
  });
  expect(running.patchbay.topology.gears.map(({ kind_id }) => kind_id)).toEqual([
    "time/every",
    "presentation/tick",
  ]);
  expect(running.patchbay.topology.cords).toHaveLength(1);
  expect(running.constraintConformance.patchbay.checked_plot_id)
    .toBe(running.constraintConformance.checkedPlotId);
  expect(running.constraintConformance.patchbay.front_inputs).toHaveLength(1);
  expect(running.constraintConformance.patchbay.front_inputs[0]).toMatchObject({
    port_id: "code",
    info_kind: "value/text",
    temporal: "value",
    value_contract: {
      value_kind: "value/text",
      maximum_bytes: 8,
      constraints: [
        { CanonicalMembership: { members: [[65, 66, 49, 50], [67, 68, 51, 52]] } },
        { TextPattern: { start_state: 0, maximum_input_characters: 8 } },
      ],
    },
  });
  const patchbay = page.locator('#patchbay [data-patchbay-schema="conduit.browser/body-patchbay@1"]');
  await expect(patchbay).toHaveAttribute("data-body-id", running.identities.bodyId);
  await expect(patchbay).toHaveAttribute("data-boot-id", running.identities.bootId);
  await expect(patchbay).toHaveAttribute("data-plan-id", running.identities.planId);
  await expect(patchbay).toHaveAttribute("data-play-id", running.identities.playId);
  await expect(patchbay.locator("[data-patchbay-gear]")).toHaveCount(2);
  await expect(patchbay.locator("[data-patchbay-cord]")).toHaveCount(1);
  for (const [name, identity] of Object.entries(running.identities)) {
    const selector = name.replace(/Id$/, "").toLowerCase();
    await expect(page.locator(`[data-identity="${selector}"]`)).toHaveText(identity);
  }

  const tick = page.locator('[data-presentation-kind="presentation/tick"]');
  await expect(tick).toHaveText("0", { timeout: 15_000 });
  const manifestation = await tick.evaluate(element => ({ ...element.dataset }));
  await page.getByRole("button", { name: "Lull Clock Body", exact: true }).click();
  await expect(page.locator("#status")).toHaveText("Clock Body lulled");
  expect(manifestation).toMatchObject({
    hostId: running.identities.hostId,
    bootId: running.identities.bootId,
    bodyPlanId: running.identities.planId,
    activePlayId: running.identities.playId,
    presentationKind: "presentation/tick",
  });
  for (const identity of [manifestation.planId, manifestation.placementId, manifestation.presentationId]) {
    expect(typeof identity === "string" && identity.length > 0).toBe(true);
  }
  const observationSequence = Number(manifestation.observationSequence);
  expect(Number.isSafeInteger(observationSequence)).toBe(true);

  const lulled = await page.evaluate(() => ({
    snapshot: __conduitFieldStation.lullSnapshot,
    playState: __conduitFieldStation.play.state,
    receipts: __conduitFieldStation.play.receipts,
  }));
  expect(lulled.playState).toBe("terminal");
  expect(lulled.receipts).toHaveLength(1);
  expect(lulled.receipts[0].active_play_id).toBe(running.identities.playId);
  expect(lulled.receipts[0].disposition).toBe("cancelled");
  expect(lulled.receipts[0].presentation_id).toBe(manifestation.presentationId);
  expect(lulled.receipts[0].timer_completions).toBeGreaterThanOrEqual(1);
  expect(lulled.receipts[0].manifestation_completions).toBeGreaterThanOrEqual(1);
  expect(lulled.receipts[0].timer_completions - lulled.receipts[0].manifestation_completions)
    .toBeGreaterThanOrEqual(0);
  expect(lulled.receipts[0].timer_completions - lulled.receipts[0].manifestation_completions)
    .toBeLessThanOrEqual(1);
  const signs = lulled.receipts[0].kernel_signs;
  expect(signs).toMatchObject({
    schema: "conduit.browser/kernel-sign-evidence@1",
    host_id: running.identities.hostId,
    boot_id: running.identities.bootId,
    active_play_id: running.identities.playId,
    retention_gap: null,
  });
  const presentation = signs.placements.find(({ placement_id }) => placement_id === manifestation.placementId);
  expect(presentation).toMatchObject({ plan_id: manifestation.planId });
  expect(signs.placements).toHaveLength(2);
  const timer = signs.placements.find(({ node }) => node !== presentation.node);
  expect(timer).toMatchObject({
    plan_id: presentation.plan_id,
    fragment_id: presentation.fragment_id,
  });
  const presentationRequest = signs.events.find(event => event.kind === "HostCallRequested"
    && event.node === presentation.node && event.request === observationSequence);
  const presentationCompletion = signs.events.find(event => event.kind === "HostCallCompleted"
    && event.node === presentation.node && event.request === observationSequence);
  expect(presentationRequest).toBeDefined();
  expect(presentationCompletion?.sequence).toBeGreaterThan(presentationRequest.sequence);
  expect(signs.events.filter(event => event.kind === "HostCallCompleted"
    && event.node === presentation.node)).toHaveLength(lulled.receipts[0].manifestation_completions);
  const timerCompletions = signs.events.filter(event => event.kind === "HostCallCompleted"
    && event.node === timer.node);
  expect(timerCompletions).toHaveLength(lulled.receipts[0].timer_completions);
  const timerCompletion = timerCompletions.findLast(event => event.sequence < presentationRequest.sequence);
  expect(timerCompletion).toBeDefined();
  const timerRequest = signs.events.find(event => event.kind === "HostCallRequested"
    && event.node === timerCompletion.node && event.request === timerCompletion.request);
  expect(timerRequest).toBeDefined();
  expect(timerCompletion.sequence).toBeGreaterThan(timerRequest.sequence);
  expect(presentationRequest.sequence).toBeGreaterThan(timerCompletion.sequence);
  const pendingTimer = signs.events.findLast(event => event.kind === "HostCallRequested"
    && event.node === timer.node
    && !signs.events.some(completion => completion.kind === "HostCallCompleted"
      && completion.node === event.node && completion.request === event.request));
  expect(pendingTimer).toBeDefined();
  const cancellationRequested = signs.events.find(event => event.kind === "CancellationRequested"
    && event.node === timer.node && event.sequence > pendingTimer.sequence);
  const runCancelled = signs.events.find(event => event.kind === "RunCancelled"
    && event.node === timer.node && event.sequence > cancellationRequested?.sequence);
  expect(cancellationRequested).toBeDefined();
  expect(runCancelled).toBeDefined();
  expect(signs.events.some(event => event.kind === "HostCallCompleted"
    && event.node === timer.node && event.request === pendingTimer.request)).toBe(false);
  expect(signs.events.some(event => event.kind === "HostCallCompleted"
    && event.sequence > runCancelled.sequence)).toBe(false);
  expect(signs.host_completions).toMatchObject({ omitted: 0 });
  expect(signs.host_completions.records).toHaveLength(
    lulled.receipts[0].timer_completions + lulled.receipts[0].manifestation_completions,
  );
  expect(signs.host_completions.records.every(record => record.disposition === "completed")).toBe(true);
  expect(signs.host_completions.records.some(record => record.node === timer.node
    && record.request === pendingTimer.request)).toBe(false);
  expect(lulled.snapshot.evidence.body_id).toBe(running.identities.bodyId);
  expect(lulled.snapshot.evidence.body.state).toBe("Lulled");
  const wake = lulled.snapshot.evidence.wakes.find(({ wake_id }) => wake_id === running.identities.wakeId);
  expect(wake?.lifecycle).toBe("Lulled");
  expect(wake?.plans).toContainEqual(expect.objectContaining({
    plan_id: running.identities.planId,
    active_play_id: running.identities.playId,
  }));
  expect(pageErrors).toEqual([]);
});
