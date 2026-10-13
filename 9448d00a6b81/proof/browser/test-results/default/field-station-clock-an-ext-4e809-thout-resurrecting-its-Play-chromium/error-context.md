# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: field-station-clock.spec.mjs >> an external page recovers one Clock Body across a fresh Boot without resurrecting its Play
- Location: proof/browser/field-station-clock.spec.mjs:9:1

# Error details

```
Error: expect(locator).toHaveText(expected) failed

Locator:  locator('#status')
Expected: "Clock Body awake"
Received: "Preparing the Clock Body…"
Timeout:  15000ms

Call log:
  - Expect "toHaveText" with timeout 15000ms
  - waiting for locator('#status')
    33 × locator resolved to <p id="status" aria-live="polite">Preparing the Clock Body…</p>
       - unexpected value "Preparing the Clock Body…"

```

```yaml
- paragraph: Preparing the Clock Body…
```

# Test source

```ts
  1   | import { expect, test } from "@playwright/test";
  2   | import { prepareFieldStationPackage } from "./field-station-package.mjs";
  3   | 
  4   | test.beforeAll(async ({}, testInfo) => {
  5   |   testInfo.setTimeout(120_000);
  6   |   await prepareFieldStationPackage();
  7   | });
  8   | 
  9   | test("an external page recovers one Clock Body across a fresh Boot without resurrecting its Play", async ({ page }) => {
  10  |   const pageErrors = [];
  11  |   page.on("pageerror", (error) => pageErrors.push(error.message));
  12  |   await page.goto("/proof/browser/field-station/");
> 13  |   await expect(page.locator("#status")).toHaveText("Clock Body awake", { timeout: 15_000 });
      |                                         ^ Error: expect(locator).toHaveText(expected) failed
  14  | 
  15  |   const first = await page.evaluate(() => ({
  16  |     identities: __conduitFieldStation.identities,
  17  |     playState: __conduitFieldStation.play.state,
  18  |     recovered: __conduitFieldStation.recovered,
  19  |   }));
  20  |   expect(first.recovered).toBe(false);
  21  |   expect(first.playState).toBe("playing");
  22  |   await expect(page.locator('[data-presentation-kind="presentation/tick"]')).toHaveText("0", { timeout: 15_000 });
  23  | 
  24  |   await page.reload();
  25  |   await expect(page.locator("#status")).toHaveText("Clock Body awake", { timeout: 15_000 });
  26  |   const running = await page.evaluate(() => ({
  27  |     identities: __conduitFieldStation.identities,
  28  |     playState: __conduitFieldStation.play.state,
  29  |     recovered: __conduitFieldStation.recovered,
  30  |     recoverySnapshot: __conduitFieldStation.recoverySnapshot,
  31  |     patchbay: __conduitFieldStation.patchbay,
  32  |   }));
  33  |   expect(running.recovered).toBe(true);
  34  |   expect(running.playState).toBe("playing");
  35  |   expect(running.identities.hostId).toBe(first.identities.hostId);
  36  |   expect(running.identities.bodyId).toBe(first.identities.bodyId);
  37  |   expect(running.identities.bootId).not.toBe(first.identities.bootId);
  38  |   expect(running.identities.wakeId).not.toBe(first.identities.wakeId);
  39  |   expect(running.identities.planId).not.toBe(first.identities.planId);
  40  |   expect(running.identities.playId).not.toBe(first.identities.playId);
  41  |   expect(running.recoverySnapshot.evidence.body_id).toBe(first.identities.bodyId);
  42  |   expect(running.recoverySnapshot.realization).toBeNull();
  43  |   expect(Object.values(running.identities).every((identity) => typeof identity === "string" && identity.length > 0)).toBe(true);
  44  |   expect(new Set(Object.values(running.identities)).size).toBe(6);
  45  |   expect(running.patchbay).toMatchObject({
  46  |     schema: "conduit.browser/body-patchbay@1",
  47  |     bodyId: running.identities.bodyId,
  48  |     hostId: running.identities.hostId,
  49  |     bootId: running.identities.bootId,
  50  |     planId: running.identities.planId,
  51  |     playId: running.identities.playId,
  52  |     topology: {
  53  |       schema: "conduit.patchbay/checked-form-projection@1",
  54  |       form_name: "clock-demo",
  55  |     },
  56  |   });
  57  |   expect(running.patchbay.topology.gears.map(({ kind_id }) => kind_id)).toEqual([
  58  |     "time/every",
  59  |     "presentation/tick",
  60  |   ]);
  61  |   expect(running.patchbay.topology.cords).toHaveLength(1);
  62  |   const patchbay = page.locator('#patchbay [data-patchbay-schema="conduit.browser/body-patchbay@1"]');
  63  |   await expect(patchbay).toHaveAttribute("data-body-id", running.identities.bodyId);
  64  |   await expect(patchbay).toHaveAttribute("data-boot-id", running.identities.bootId);
  65  |   await expect(patchbay).toHaveAttribute("data-plan-id", running.identities.planId);
  66  |   await expect(patchbay).toHaveAttribute("data-play-id", running.identities.playId);
  67  |   await expect(patchbay.locator("[data-patchbay-gear]")).toHaveCount(2);
  68  |   await expect(patchbay.locator("[data-patchbay-cord]")).toHaveCount(1);
  69  |   for (const [name, identity] of Object.entries(running.identities)) {
  70  |     const selector = name.replace(/Id$/, "").toLowerCase();
  71  |     await expect(page.locator(`[data-identity="${selector}"]`)).toHaveText(identity);
  72  |   }
  73  | 
  74  |   const tick = page.locator('[data-presentation-kind="presentation/tick"]');
  75  |   await expect(tick).toHaveText("0", { timeout: 15_000 });
  76  |   const manifestation = await tick.evaluate(element => ({ ...element.dataset }));
  77  |   await page.getByRole("button", { name: "Lull Clock Body", exact: true }).click();
  78  |   await expect(page.locator("#status")).toHaveText("Clock Body lulled");
  79  |   expect(manifestation).toMatchObject({
  80  |     hostId: running.identities.hostId,
  81  |     bootId: running.identities.bootId,
  82  |     bodyPlanId: running.identities.planId,
  83  |     activePlayId: running.identities.playId,
  84  |     presentationKind: "presentation/tick",
  85  |   });
  86  |   for (const identity of [manifestation.planId, manifestation.placementId, manifestation.presentationId]) {
  87  |     expect(typeof identity === "string" && identity.length > 0).toBe(true);
  88  |   }
  89  |   const observationSequence = Number(manifestation.observationSequence);
  90  |   expect(Number.isSafeInteger(observationSequence)).toBe(true);
  91  | 
  92  |   const lulled = await page.evaluate(() => ({
  93  |     snapshot: __conduitFieldStation.lullSnapshot,
  94  |     playState: __conduitFieldStation.play.state,
  95  |     receipts: __conduitFieldStation.play.receipts,
  96  |   }));
  97  |   expect(lulled.playState).toBe("terminal");
  98  |   expect(lulled.receipts).toHaveLength(1);
  99  |   expect(lulled.receipts[0].active_play_id).toBe(running.identities.playId);
  100 |   expect(lulled.receipts[0].disposition).toBe("cancelled");
  101 |   expect(lulled.receipts[0].presentation_id).toBe(manifestation.presentationId);
  102 |   expect(lulled.receipts[0].timer_completions).toBeGreaterThanOrEqual(1);
  103 |   expect(lulled.receipts[0].manifestation_completions).toBeGreaterThanOrEqual(1);
  104 |   expect(lulled.receipts[0].timer_completions - lulled.receipts[0].manifestation_completions)
  105 |     .toBeGreaterThanOrEqual(0);
  106 |   expect(lulled.receipts[0].timer_completions - lulled.receipts[0].manifestation_completions)
  107 |     .toBeLessThanOrEqual(1);
  108 |   const signs = lulled.receipts[0].kernel_signs;
  109 |   expect(signs).toMatchObject({
  110 |     schema: "conduit.browser/kernel-sign-evidence@1",
  111 |     host_id: running.identities.hostId,
  112 |     boot_id: running.identities.bootId,
  113 |     active_play_id: running.identities.playId,
```