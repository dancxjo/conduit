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
  expect(manifestation).toMatchObject({
    hostId: running.identities.hostId,
    bootId: running.identities.bootId,
    activePlayId: running.identities.playId,
    presentationKind: "presentation/tick",
  });
  for (const identity of [manifestation.planId, manifestation.placementId, manifestation.presentationId]) {
    expect(typeof identity === "string" && identity.length > 0).toBe(true);
  }
  const observationSequence = Number(manifestation.observationSequence);
  expect(Number.isSafeInteger(observationSequence)).toBe(true);

  await page.getByRole("button", { name: "Lull Clock Body", exact: true }).click();
  await expect(page.locator("#status")).toHaveText("Clock Body lulled");
  const lulled = await page.evaluate(() => ({
    snapshot: __conduitFieldStation.lullSnapshot,
    playState: __conduitFieldStation.play.state,
    receipts: __conduitFieldStation.play.receipts,
  }));
  expect(lulled.playState).toBe("terminal");
  expect(lulled.receipts).toHaveLength(1);
  expect(lulled.receipts[0].active_play_id).toBe(running.identities.playId);
  expect(lulled.receipts[0].disposition).toBe("cancelled");
  expect(lulled.receipts[0].timer_completions).toBe(1);
  expect(lulled.receipts[0].manifestation_completions).toBe(1);
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
  const presentationRequest = signs.events.find(event => event.kind === "HostCallRequested"
    && event.node === presentation.node && event.request === observationSequence);
  const presentationCompletion = signs.events.find(event => event.kind === "HostCallCompleted"
    && event.node === presentation.node && event.request === observationSequence);
  expect(presentationRequest).toBeDefined();
  expect(presentationCompletion?.sequence).toBeGreaterThan(presentationRequest.sequence);
  const timerCompletions = signs.events.filter(event => event.kind === "HostCallCompleted"
    && event.node !== presentation.node);
  expect(timerCompletions).toHaveLength(1);
  const timerCompletion = timerCompletions[0];
  const timerRequest = signs.events.find(event => event.kind === "HostCallRequested"
    && event.node === timerCompletion.node && event.request === timerCompletion.request);
  expect(timerRequest).toBeDefined();
  expect(timerCompletion.sequence).toBeGreaterThan(timerRequest.sequence);
  expect(presentationRequest.sequence).toBeGreaterThan(timerCompletion.sequence);
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
