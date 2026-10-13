# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: workspace-library.spec.mjs >> Use installs into the same body; repeated Use preserves Play and removal survives reload
- Location: proof/browser/workspace-library.spec.mjs:57:1

# Error details

```
Test timeout of 45000ms exceeded.
```

```
Error: expect(locator).toContainText(expected) failed

Locator: locator('[data-application-key^="library-plot-"]').filter({ hasText: /^Desk Telegraph/ })
Expected substring: "Not in your body"
Received string:    "Desk TelegraphIn your bodyUseRemove"
Timeout: 5000ms

Call log:
  - Expect "toContainText" with timeout 5000ms
  - waiting for locator('[data-application-key^="library-plot-"]').filter({ hasText: /^Desk Telegraph/ })
    - locator resolved to <section data-application-component="panel" data-application-key="library-plot-23">…</section>
    - unexpected value "Desk TelegraphIn your bodyUseRemove"

```

```yaml
- text: Desk Telegraph
- status: In your body
- group "Desk Telegraph actions":
  - button "Use"
  - button "Remove"
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
  19  |   await chime.uncheck();
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
> 97  |   await expect(card(page, "Desk Telegraph")).toContainText("Not in your body");
      |                                              ^ Error: expect(locator).toContainText(expected) failed
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
  120 |         return super.createBuffer(channels, frames, sampleRate);
  121 |       }
  122 |       createBufferSource() {
  123 |         const source = super.createBufferSource();
  124 |         const start = source.start.bind(source);
  125 |         source.start = when => {
  126 |           const samples = source.buffer.getChannelData(0);
  127 |           let peak = 0, crossings = 0;
  128 |           for (let index = 0; index < samples.length; index += 1) {
  129 |             peak = Math.max(peak, Math.abs(samples[index]));
  130 |             if (index > 0 && samples[index - 1] <= 0 && samples[index] > 0) crossings += 1;
  131 |           }
  132 |           globalThis.__thereminAudio.starts.push({ when, peak, crossings });
  133 |           return start(when);
  134 |         };
  135 |         return source;
  136 |       }
  137 |     };
  138 |   });
  139 |   await birth(page);
  140 |   await openLibrary(page);
  141 |   await page.getByRole("textbox", { name: "Find a plot", exact: true }).fill("theremin");
  142 |   await card(page, "Pocket Theremin").getByRole("button", { name: "Use", exact: true }).click();
  143 |   await expect(page.locator("#surface-title")).toHaveText("Pocket Theremin");
  144 |   const surface = page.locator("#plot-input[data-pocket-theremin]");
  145 |   await expect(surface).toBeVisible();
  146 |   // Raw mouse coordinates do not perform locator scrolling. Bring the surface
  147 |   // into view and check every gesture extreme before starting pointer input.
  148 |   await surface.scrollIntoViewIfNeeded();
  149 |   const bounds = await surface.boundingBox();
  150 |   const viewport = page.viewportSize();
  151 |   expect(bounds).not.toBeNull();
  152 |   expect(bounds.x + bounds.width * 0.15).toBeGreaterThanOrEqual(0);
  153 |   expect(bounds.y + bounds.height * 0.15).toBeGreaterThanOrEqual(0);
  154 |   expect(bounds.x + bounds.width * 0.98).toBeLessThan(viewport.width);
  155 |   expect(bounds.y + bounds.height * 0.85).toBeLessThan(viewport.height);
  156 |   await page.mouse.move(bounds.x + bounds.width * 0.15, bounds.y + bounds.height * 0.85);
  157 |   await page.mouse.down();
  158 |   await expect.poll(() => page.evaluate(() => globalThis.__thereminAudio.starts.length)).toBeGreaterThan(2);
  159 |   const highPitchEnd = await page.evaluate(() => globalThis.__thereminAudio.starts.length);
  160 |   await page.mouse.move(bounds.x + bounds.width * 0.98, bounds.y + bounds.height * 0.85, { steps: 8 });
  161 |   await expect.poll(() => page.evaluate(end => globalThis.__thereminAudio.starts.slice(end + 1).some(block => block.crossings <= 10), highPitchEnd)).toBe(true);
  162 |   const loudEnd = await page.evaluate(() => globalThis.__thereminAudio.starts.length);
  163 |   const loudPeak = await page.evaluate(({ start, end }) => Math.max(...globalThis.__thereminAudio.starts.slice(start + 1, end).map(block => block.peak)), { start: highPitchEnd, end: loudEnd });
  164 |   await page.mouse.move(bounds.x + bounds.width * 0.98, bounds.y + bounds.height * 0.15, { steps: 8 });
  165 |   await expect.poll(() => page.evaluate(({ end, threshold }) => globalThis.__thereminAudio.starts.slice(end + 1).some(block => block.peak < threshold), { end: loudEnd, threshold: loudPeak * 0.4 })).toBe(true);
  166 |   const audio = await page.evaluate(() => globalThis.__thereminAudio);
  167 |   expect(audio.blocks.every(block => block.channels === 1 && block.frames === 2000 && block.sampleRate === 48000)).toBe(true);
  168 |   expect(audio.starts[1].when).toBeGreaterThanOrEqual(audio.starts[0].when);
  169 |   expect(audio.starts.slice(1, highPitchEnd).some(block => block.crossings >= 20)).toBe(true);
  170 |   // 110 Hz spans 4.58 cycles in each 2,000-frame block, so phase-continuous
  171 |   // blocks contain either nine or ten zero crossings.
  172 |   expect(audio.starts.slice(highPitchEnd + 1, loudEnd).some(block => block.crossings <= 10)).toBe(true);
  173 |   const quietPeak = Math.min(...audio.starts.slice(loudEnd + 1).map(block => block.peak));
  174 |   expect(quietPeak).toBeLessThan(loudPeak * 0.4);
  175 |   await page.mouse.up();
  176 | });
  177 | 
  178 | test("the reviewed shelf reveals conversation Plots without pretending this browser can run them", async ({ page }) => {
  179 |   await birth(page);
  180 |   await openLibrary(page);
  181 |   await page.getByRole("textbox", { name: "Find a plot", exact: true }).fill("conversation");
  182 |   await expect(card(page, "Body Chat")).toContainText("Needs capability");
  183 |   await expect(card(page, "Body Chat")).toContainText("current body supervisor and an admitted model realization");
  184 |   await expect(card(page, "Live Conversation")).toContainText("Needs capability");
  185 |   await expect(card(page, "Live Conversation")).toContainText("live audio acquisition, streaming model generation, synthesis, and presentation");
  186 |   await expect(card(page, "Live Conversation")).toContainText("Text fallback: Body Chat");
  187 |   await expect(card(page, "Live Conversation")).toContainText("The fallback still needs capability");
  188 |   await expect(card(page, "House Spoken Response")).toContainText("Text fallback: House Conversation");
  189 |   await expect(card(page, "Body Chat").getByRole("button", { name: /Use — unavailable/u })).toBeDisabled();
  190 |   await expect(card(page, "Live Conversation").getByRole("button", { name: /Use — unavailable/u })).toBeDisabled();
  191 |   await expect(page.getByText("4 Plots", { exact: true })).toBeVisible();
  192 | });
  193 | 
  194 | test("window focus loss retires the pending Play and lets the same body wake again", async ({ page }) => {
  195 |   await birth(page);
  196 |   const before = await current(page);
  197 |   await page.evaluate(() => window.dispatchEvent(new Event("blur")));
```