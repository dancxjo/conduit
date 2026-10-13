# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: workspace-library.spec.mjs >> repeated Wake and Lull compacts retained evidence instead of exhausting lifecycle Signs
- Location: proof/browser/workspace-library.spec.mjs:38:1

# Error details

```
Test timeout of 60000ms exceeded.
```

```
Error: locator.uncheck: Test timeout of 60000ms exceeded.
Call log:
  - waiting for getByRole('checkbox', { name: 'Startup Chime', exact: true })

```

# Test source

```ts
  1   | import { expect, test } from "@playwright/test";
  2   | import { startStaticProduct } from "./static-product-server.mjs";
  3   | 
  4   | let entrance, pageErrors;
  5   | test.beforeEach(async ({ page }) => { pageErrors = []; page.on("pageerror", error => pageErrors.push(error.message)); entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
  6   | test.afterEach(async ({ page }, info) => {
  7   |   if (info.status !== info.expectedStatus) console.error((await page.locator('body').innerText()).slice(0, 6000));
  8   |   entrance?.child.kill(); expect(pageErrors).toEqual([]);
  9   | });
  10  | 
  11  | const current = page => page.evaluate(() => globalThis.__conduitWorkspace.current());
  12  | const card = (page, title) => page.locator('[data-application-key^="library-plot-"]').filter({
  13  |   hasText: new RegExp(`^${title.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&")}`),
  14  | });
  15  | const openLibrary = page => page.getByRole("button", { name: "+ Plots", exact: true }).click();
  16  | async function birth(page) {
  17  |   await page.goto(entrance.url);
  18  |   const chime = page.getByRole("checkbox", { name: "Startup Chime", exact: true });
> 19  |   await chime.uncheck();
      |               ^ Error: locator.uncheck: Test timeout of 60000ms exceeded.
  20  |   await page.getByLabel("Friendly Body name", { exact: true }).fill("Roseau");
  21  |   await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  22  |   await page.getByRole("button", { name: "wake body", exact: true }).click();
  23  |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  24  | }
  25  | test("five ordinary plots start together beyond the old aggregate placement ceiling", async ({ page }) => {
  26  |   await page.goto(entrance.url);
  27  |   for (const title of ["Button Across the Room", "Firefly Choir"]) {
  28  |     await page.getByRole("checkbox", { name: title, exact: true }).check();
  29  |   }
  30  |   await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  31  |   await page.getByRole("button", { name: "wake body", exact: true }).click();
  32  |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  33  |   const state = await current(page);
  34  |   expect(state.initial_plots).toHaveLength(5);
  35  |   expect(state.refusal).toBeUndefined();
  36  | });
  37  | 
  38  | test("repeated Wake and Lull compacts retained evidence instead of exhausting lifecycle Signs", async ({ page }) => {
  39  |   // Twelve complete durable lifecycle transitions can exceed the default
  40  |   // aggregate budget on hosted runners; keep every transition assertion.
  41  |   test.setTimeout(60_000);
  42  |   await birth(page);
  43  |   const identity = (await current(page)).body_id;
  44  |   for (let index = 0; index < 12; index++) {
  45  |     await page.getByRole("button", { name: "lull body", exact: true }).click();
  46  |     await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  47  |     await page.getByRole("button", { name: "wake body", exact: true }).click();
  48  |     await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  49  |   }
  50  |   expect((await current(page)).body_id).toBe(identity);
  51  |   await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  52  |   await page.reload();
  53  |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  54  |   expect((await current(page)).body_id).toBe(identity);
  55  | });
  56  | 
  57  | test("Use installs into the same body; repeated Use preserves Play and removal survives reload", async ({ page }, testInfo) => {
  58  |   // This journey includes three workset changes, interactive execution and reload.
  59  |   // Two retained Candidate traces passed preceding assertions but exhausted the
  60  |   // default 20-second aggregate budget during the final lifecycle/reload step.
  61  |   test.setTimeout(45_000);
  62  |   await page.setViewportSize({ width: 1280, height: 1000 });
  63  |   await birth(page);
  64  |   const initial = await current(page);
  65  |   await openLibrary(page);
  66  |   await expect(card(page, "Memory Lantern")).toContainText("In your body");
  67  |   await page.screenshot({ path: testInfo.outputPath("workspace-plot-library.png"), fullPage: true });
  68  |   await page.getByRole("textbox", { name: "Find a plot", exact: true }).fill("desk");
  69  |   await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  70  |   await expect(page.locator("#surface-title")).toHaveText("Desk Telegraph");
  71  |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  72  |   const installed = await current(page);
  73  |   expect(installed.body_id).toBe(initial.body_id);
  74  |   expect(installed.here_part_id).toBe(initial.here_part_id);
  75  |   expect(installed.workload_revision).toBe(1);
  76  |   expect(installed.active_play_id).not.toBe(initial.active_play_id);
  77  |   expect(installed.initial_plots).toHaveLength(3);
  78  |   const tutorial = page.locator('[data-body-tutorial]');
  79  |   await expect(tutorial).toContainText("Invite another host");
  80  |   await tutorial.getByRole("button", { name: "Invite another host", exact: true }).click();
  81  |   await expect(page.getByRole("heading", { name: "parts and hosts", exact: true })).toBeVisible();
  82  |   await page.getByRole("button", { name: "back to the surface", exact: true }).click();
  83  |   await page.locator("#plot-input").focus();
  84  |   await page.keyboard.type("hello");
  85  |   await page.keyboard.press("Enter");
  86  |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("hello");
  87  |   await page.keyboard.type("again");
  88  |   await page.keyboard.press("Enter");
  89  |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("again");
  90  |   await openLibrary(page);
  91  |   await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  92  |   expect((await current(page)).active_play_id).toBe(installed.active_play_id);
  93  |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("again");
  94  |   await page.screenshot({ path: testInfo.outputPath("workspace-installed-plot.png"), fullPage: true });
  95  |   await openLibrary(page);
  96  |   await card(page, "Desk Telegraph").getByRole("button", { name: "Remove", exact: true }).click();
  97  |   await expect(card(page, "Desk Telegraph")).toContainText("Not in your body");
  98  |   await page.getByRole("button", { name: "back to the surface", exact: true }).click();
  99  |   await expect(page.locator("#surface-title")).toHaveText("Memory Lantern");
  100 |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  101 |   await page.locator("#plot-input").focus();
  102 |   await page.keyboard.press("r");
  103 |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("r");
  104 |   await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  105 |   await page.reload();
  106 |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  107 |   const restored = await current(page);
  108 |   expect(restored.body_id).toBe(initial.body_id);
  109 |   expect(restored.workload_revision).toBe(2);
  110 |   expect(restored.initial_plots).toEqual(initial.initial_plots);
  111 | });
  112 | 
  113 | test("Pocket Theremin plays continuous two-axis PCM on the browser host", async ({ page }) => {
  114 |   await page.addInitScript(() => {
  115 |     const NativeAudioContext = window.AudioContext;
  116 |     globalThis.__thereminAudio = { blocks: [], starts: [] };
  117 |     window.AudioContext = class extends NativeAudioContext {
  118 |       createBuffer(channels, frames, sampleRate) {
  119 |         globalThis.__thereminAudio.blocks.push({ channels, frames, sampleRate });
```