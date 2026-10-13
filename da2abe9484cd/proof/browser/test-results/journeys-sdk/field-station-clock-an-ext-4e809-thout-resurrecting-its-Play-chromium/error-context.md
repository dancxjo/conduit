# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: field-station-clock.spec.mjs >> an external page recovers one Clock Body across a fresh Boot without resurrecting its Play
- Location: proof/browser/field-station-clock.spec.mjs:10:1

# Error details

```
Error: expect(received).toMatchObject(expected)

- Expected  -  0
+ Received  + 52

@@ -22,12 +22,64 @@
            ],
          },
        },
        Object {
          "TextPattern": Object {
+           "anchored_end": true,
+           "anchored_start": true,
+           "negated": false,
+           "pattern": Object {
              "maximum_input_characters": 8,
+             "maximum_match_steps": 36,
              "start_state": 0,
+             "states": Array [
+               Object {
+                 "accepting": false,
+                 "transitions": Array [
+                   Object {
+                     "first_scalar": 65,
+                     "last_scalar": 90,
+                     "target_state": 1,
+                   },
+                 ],
+               },
+               Object {
+                 "accepting": false,
+                 "transitions": Array [
+                   Object {
+                     "first_scalar": 65,
+                     "last_scalar": 90,
+                     "target_state": 2,
+                   },
+                 ],
+               },
+               Object {
+                 "accepting": false,
+                 "transitions": Array [
+                   Object {
+                     "first_scalar": 48,
+                     "last_scalar": 57,
+                     "target_state": 3,
+                   },
+                 ],
+               },
+               Object {
+                 "accepting": false,
+                 "transitions": Array [
+                   Object {
+                     "first_scalar": 48,
+                     "last_scalar": 57,
+                     "target_state": 4,
+                   },
+                 ],
+               },
+               Object {
+                 "accepting": true,
+                 "transitions": Array [],
+               },
+             ],
+           },
          },
        },
      ],
      "maximum_bytes": 8,
      "value_kind": "value/text",
```

# Page snapshot

```yaml
- main [ref=f1e2]:
  - heading "Field Station" [level=1] [ref=f1e3]
  - paragraph [ref=f1e4]: Clock Body awake
  - region "Clock presentation":
    - status "clock-demo/inline-ced6293e47da-0"
  - region "Clock Patchbay" [ref=f1e5]:
    - region "Patchbay checked Plot topology" [ref=f1e6]:
      - heading "Patchbay — clock-demo" [level=2] [ref=f1e7]
      - list "Gears" [ref=f1e8]:
        - listitem [ref=f1e9]: clock-demo/clock — time/every
        - listitem [ref=f1e10]: clock-demo/inline-ced6293e47da-0 — presentation/tick
      - list "Cords" [ref=f1e11]:
        - listitem [ref=f1e12]: clock-demo/clock.tick → clock-demo/inline-ced6293e47da-0.tick (value/tick@1, flow-open)
  - generic [ref=f1e13]:
    - term [ref=f1e14]: Host
    - definition [ref=f1e15]: browser/cc7da311-dbbc-4eea-81e5-f8c998b1591f
    - term [ref=f1e16]: Boot
    - definition [ref=f1e17]: browser-boot/a4ce6319-9399-462e-9085-80ff4b7afa0a
    - term [ref=f1e18]: Body
    - definition [ref=f1e19]: d1dc6467d82671cf0b2d9fb783ca44f867091f0f32729731bc2e37790107b37e
    - term [ref=f1e20]: Wake
    - definition [ref=f1e21]: 4c49ff021d18950f3aaea0593a0cd3f27791c275b0f45579094668b69f6e250d
    - term [ref=f1e22]: Plan
    - definition [ref=f1e23]: body-plan/sha256:1e46aac1fc42f96d9fd3888b131557e0baadff4ab18753d14c70b87464034536
    - term [ref=f1e24]: Play
    - definition [ref=f1e25]: body-play/sha256:e6caebef78bab76b60fd41e8a0ef55a74cc3d931e7b676847579a3675919d65e
  - button "Lull Clock Body" [ref=f1e26]
```

# Test source

```ts
  1   | import { expect, test } from "@playwright/test";
  2   | import { prepareFieldStationPackage } from "./field-station-package.mjs";
  3   | import { beginFieldStationJourney } from "./field-station-journey.mjs";
  4   | 
  5   | test.beforeAll(async ({}, testInfo) => {
  6   |   testInfo.setTimeout(120_000);
  7   |   await prepareFieldStationPackage();
  8   | });
  9   | 
  10  | test("an external page recovers one Clock Body across a fresh Boot without resurrecting its Play", async ({ page }) => {
  11  |   const journey = await beginFieldStationJourney();
  12  |   const pageErrors = [];
  13  |   page.on("pageerror", (error) => pageErrors.push(error.message));
  14  |   await page.goto("/proof/browser/field-station/");
  15  |   await expect(page.locator("#status")).toHaveText("Clock Body awake", { timeout: 15_000 });
  16  | 
  17  |   const first = await page.evaluate(() => ({
  18  |     identities: __conduitFieldStation.identities,
  19  |     playState: __conduitFieldStation.play.state,
  20  |     recovered: __conduitFieldStation.recovered,
  21  |   }));
  22  |   expect(first.recovered).toBe(false);
  23  |   expect(first.playState).toBe("playing");
  24  |   await expect(page.locator('[data-presentation-kind="presentation/tick"]')).toHaveText("0", { timeout: 15_000 });
  25  | 
  26  |   await journey.capture(page, {
  27  |     id: "open", title: "Open the clock",
  28  |     action: "Open the Field Station page.",
  29  |     observation: "The page automatically creates and wakes a Clock Body. Its first tick is visible, alongside the Clock Patchbay and exact runtime identities.",
  30  |   });
  31  | 
  32  |   await page.reload();
  33  |   await expect(page.locator("#status")).toHaveText("Clock Body awake", { timeout: 15_000 });
  34  |   const running = await page.evaluate(() => ({
  35  |     identities: __conduitFieldStation.identities,
  36  |     playState: __conduitFieldStation.play.state,
  37  |     recovered: __conduitFieldStation.recovered,
  38  |     recoverySnapshot: __conduitFieldStation.recoverySnapshot,
  39  |     patchbay: __conduitFieldStation.patchbay,
  40  |     constraintConformance: __conduitFieldStation.constraintConformance,
  41  |   }));
  42  |   expect(running.recovered).toBe(true);
  43  |   expect(running.playState).toBe("playing");
  44  |   expect(running.identities.hostId).toBe(first.identities.hostId);
  45  |   expect(running.identities.bodyId).toBe(first.identities.bodyId);
  46  |   expect(running.identities.bootId).not.toBe(first.identities.bootId);
  47  |   expect(running.identities.wakeId).not.toBe(first.identities.wakeId);
  48  |   expect(running.identities.planId).not.toBe(first.identities.planId);
  49  |   expect(running.identities.playId).not.toBe(first.identities.playId);
  50  |   expect(running.recoverySnapshot.evidence.body_id).toBe(first.identities.bodyId);
  51  |   expect(running.recoverySnapshot.realization).toBeNull();
  52  |   expect(Object.values(running.identities).every((identity) => typeof identity === "string" && identity.length > 0)).toBe(true);
  53  |   expect(new Set(Object.values(running.identities)).size).toBe(6);
  54  |   expect(running.patchbay).toMatchObject({
  55  |     schema: "conduit.browser/body-patchbay@1",
  56  |     bodyId: running.identities.bodyId,
  57  |     hostId: running.identities.hostId,
  58  |     bootId: running.identities.bootId,
  59  |     planId: running.identities.planId,
  60  |     playId: running.identities.playId,
  61  |     topology: {
  62  |       schema: "conduit.patchbay/checked-plot-projection@1",
  63  |       plot_name: "clock-demo",
  64  |     },
  65  |   });
  66  |   expect(running.patchbay.topology.gears.map(({ kind_id }) => kind_id)).toEqual([
  67  |     "time/every",
  68  |     "presentation/tick",
  69  |   ]);
  70  |   expect(running.patchbay.topology.cords).toHaveLength(1);
  71  |   expect(running.constraintConformance.patchbay.checked_plot_id)
  72  |     .toBe(running.constraintConformance.checkedPlotId);
  73  |   expect(running.constraintConformance.patchbay.front_inputs).toHaveLength(1);
> 74  |   expect(running.constraintConformance.patchbay.front_inputs[0]).toMatchObject({
      |                                                                  ^ Error: expect(received).toMatchObject(expected)
  75  |     port_id: "code",
  76  |     info_kind: "value/text",
  77  |     temporal: "value",
  78  |     value_contract: {
  79  |       value_kind: "value/text",
  80  |       maximum_bytes: 8,
  81  |       constraints: [
  82  |         { CanonicalMembership: { members: [[65, 66, 49, 50], [67, 68, 51, 52]] } },
  83  |         { TextPattern: { start_state: 0, maximum_input_characters: 8 } },
  84  |       ],
  85  |     },
  86  |   });
  87  |   const patchbay = page.locator('#patchbay [data-patchbay-schema="conduit.browser/body-patchbay@1"]');
  88  |   await expect(patchbay).toHaveAttribute("data-body-id", running.identities.bodyId);
  89  |   await expect(patchbay).toHaveAttribute("data-boot-id", running.identities.bootId);
  90  |   await expect(patchbay).toHaveAttribute("data-plan-id", running.identities.planId);
  91  |   await expect(patchbay).toHaveAttribute("data-play-id", running.identities.playId);
  92  |   await expect(patchbay.locator("[data-patchbay-gear]")).toHaveCount(2);
  93  |   await expect(patchbay.locator("[data-patchbay-cord]")).toHaveCount(1);
  94  |   for (const [name, identity] of Object.entries(running.identities)) {
  95  |     const selector = name.replace(/Id$/, "").toLowerCase();
  96  |     await expect(page.locator(`[data-identity="${selector}"]`)).toHaveText(identity);
  97  |   }
  98  | 
  99  |   const tick = page.locator('[data-presentation-kind="presentation/tick"]');
  100 |   await expect(tick).toHaveText("0", { timeout: 15_000 });
  101 |   const manifestation = await tick.evaluate(element => ({ ...element.dataset }));
  102 |   await journey.capture(page, {
  103 |     id: "reload", title: "Reload the same Body",
  104 |     action: "Reload the page using the browser.",
  105 |     observation: "The clock is awake again. The Body and Host identities stay the same; the Boot, Wake, Plan, and Play identities are new. The page recovers the Body and starts a fresh Play.",
  106 |   });
  107 |   await page.getByRole("button", { name: "Lull Clock Body", exact: true }).click();
  108 |   await expect(page.locator("#status")).toHaveText("Clock Body lulled");
  109 |   expect(manifestation).toMatchObject({
  110 |     hostId: running.identities.hostId,
  111 |     bootId: running.identities.bootId,
  112 |     bodyPlanId: running.identities.planId,
  113 |     activePlayId: running.identities.playId,
  114 |     presentationKind: "presentation/tick",
  115 |   });
  116 |   for (const identity of [manifestation.planId, manifestation.placementId, manifestation.presentationId]) {
  117 |     expect(typeof identity === "string" && identity.length > 0).toBe(true);
  118 |   }
  119 |   const observationSequence = Number(manifestation.observationSequence);
  120 |   expect(Number.isSafeInteger(observationSequence)).toBe(true);
  121 | 
  122 |   const lulled = await page.evaluate(() => ({
  123 |     snapshot: __conduitFieldStation.lullSnapshot,
  124 |     playState: __conduitFieldStation.play.state,
  125 |     receipts: __conduitFieldStation.play.receipts,
  126 |   }));
  127 |   expect(lulled.playState).toBe("terminal");
  128 |   expect(lulled.receipts).toHaveLength(1);
  129 |   expect(lulled.receipts[0].active_play_id).toBe(running.identities.playId);
  130 |   expect(lulled.receipts[0].disposition).toBe("cancelled");
  131 |   expect(lulled.receipts[0].presentation_id).toBe(manifestation.presentationId);
  132 |   expect(lulled.receipts[0].timer_completions).toBeGreaterThanOrEqual(1);
  133 |   expect(lulled.receipts[0].manifestation_completions).toBeGreaterThanOrEqual(1);
  134 |   expect(lulled.receipts[0].timer_completions - lulled.receipts[0].manifestation_completions)
  135 |     .toBeGreaterThanOrEqual(0);
  136 |   expect(lulled.receipts[0].timer_completions - lulled.receipts[0].manifestation_completions)
  137 |     .toBeLessThanOrEqual(1);
  138 |   const signs = lulled.receipts[0].kernel_signs;
  139 |   expect(signs).toMatchObject({
  140 |     schema: "conduit.browser/kernel-sign-evidence@1",
  141 |     host_id: running.identities.hostId,
  142 |     boot_id: running.identities.bootId,
  143 |     active_play_id: running.identities.playId,
  144 |     retention_gap: null,
  145 |   });
  146 |   const presentation = signs.placements.find(({ placement_id }) => placement_id === manifestation.placementId);
  147 |   expect(presentation).toMatchObject({ plan_id: manifestation.planId });
  148 |   expect(signs.placements).toHaveLength(2);
  149 |   const timer = signs.placements.find(({ node }) => node !== presentation.node);
  150 |   expect(timer).toMatchObject({
  151 |     plan_id: presentation.plan_id,
  152 |     fragment_id: presentation.fragment_id,
  153 |   });
  154 |   const presentationRequest = signs.events.find(event => event.kind === "HostCallRequested"
  155 |     && event.node === presentation.node && event.request === observationSequence);
  156 |   const presentationCompletion = signs.events.find(event => event.kind === "HostCallCompleted"
  157 |     && event.node === presentation.node && event.request === observationSequence);
  158 |   expect(presentationRequest).toBeDefined();
  159 |   expect(presentationCompletion?.sequence).toBeGreaterThan(presentationRequest.sequence);
  160 |   expect(signs.events.filter(event => event.kind === "HostCallCompleted"
  161 |     && event.node === presentation.node)).toHaveLength(lulled.receipts[0].manifestation_completions);
  162 |   const timerCompletions = signs.events.filter(event => event.kind === "HostCallCompleted"
  163 |     && event.node === timer.node);
  164 |   expect(timerCompletions).toHaveLength(lulled.receipts[0].timer_completions);
  165 |   const timerCompletion = timerCompletions.findLast(event => event.sequence < presentationRequest.sequence);
  166 |   expect(timerCompletion).toBeDefined();
  167 |   const timerRequest = signs.events.find(event => event.kind === "HostCallRequested"
  168 |     && event.node === timerCompletion.node && event.request === timerCompletion.request);
  169 |   expect(timerRequest).toBeDefined();
  170 |   expect(timerCompletion.sequence).toBeGreaterThan(timerRequest.sequence);
  171 |   expect(presentationRequest.sequence).toBeGreaterThan(timerCompletion.sequence);
  172 |   const pendingTimer = signs.events.findLast(event => event.kind === "HostCallRequested"
  173 |     && event.node === timer.node
  174 |     && !signs.events.some(completion => completion.kind === "HostCallCompleted"
```