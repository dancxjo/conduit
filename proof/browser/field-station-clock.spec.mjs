import { expect, test } from "@playwright/test";
import { prepareFieldStationPackage } from "./field-station-package.mjs";

test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(120_000);
  await prepareFieldStationPackage();
});

test("an external page births, wakes, inspects, and lulls the canonical Clock through the public SDK", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto("/proof/browser/field-station/");
  await expect(page.locator("#status")).toHaveText("Clock Body awake", { timeout: 15_000 });

  const running = await page.evaluate(() => ({
    identities: __conduitFieldStation.identities,
    playState: __conduitFieldStation.play.state,
  }));
  expect(running.playState).toBe("playing");
  expect(Object.values(running.identities).every((identity) => typeof identity === "string" && identity.length > 0)).toBe(true);
  expect(new Set(Object.values(running.identities)).size).toBe(6);
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
