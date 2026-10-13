# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: workspace-library.spec.mjs >> Pocket Theremin plays continuous two-axis PCM on the browser host
- Location: proof/browser/workspace-library.spec.mjs:106:1

# Error details

```
Error: expect(received).toBeLessThanOrEqual(expected)

Expected: <= 720
Received:    720.75
```

# Page snapshot

```yaml
- generic [ref=e1]:
  - banner [ref=e2]:
    - link "Conduit home" [ref=e3] [cursor=pointer]:
      - /url: /conduit/
      - text: Conduit
    - navigation "Main navigation" [ref=e4]:
      - link "Home" [ref=e5] [cursor=pointer]:
        - /url: /conduit/
      - link "Journeys" [ref=e6] [cursor=pointer]:
        - /url: /conduit/journeys/
      - link "Handbook" [ref=e7] [cursor=pointer]:
        - /url: /conduit/handbook/
      - link "Downloads" [ref=e8] [cursor=pointer]:
        - /url: /conduit/#get-conduit
      - link "Open Conduit" [ref=e9] [cursor=pointer]:
        - /url: /conduit/workspace/
    - link "Source" [ref=e10] [cursor=pointer]:
      - /url: https://github.com/dancxjo/conduit
  - link "Skip to the plot" [ref=e11] [cursor=pointer]:
    - /url: "#plot-input"
  - generic [ref=e12]:
    - banner [ref=e13]:
      - link "Conduit home" [ref=e14] [cursor=pointer]:
        - /url: ../
        - text: c. conduit
      - generic [ref=e15]:
        - heading "Roseau" [level=1] [ref=e16]
        - text: awake
        - generic [ref=e17]:
          - button "parts / hosts" [ref=e18] [cursor=pointer]
          - button "Invite another host" [ref=e19] [cursor=pointer]
    - group [ref=e20]:
      - generic "Learn through this body" [ref=e21] [cursor=pointer]
      - status "tour/view" [ref=e23]:
        - generic [ref=e24]:
          - text: Invite another host
          - status [ref=e25]: Tutorial · add-host — The workset changed without rebirth. Invite another host through the same finite Body admission path, then inspect its exact membership and offers.
          - button "Invite another host" [ref=e26] [cursor=pointer]
          - status [ref=e27]: Purpose · exact readiness — not ready · 2 exact obligation(s) remain
    - main [ref=e28]:
      - region [ref=e29]:
        - generic [ref=e30]:
          - heading "Pocket Theremin" [level=2] [ref=e31]
          - paragraph [ref=e32]: Glide left ↔ right for pitch. Move up toward the volume loop to mute; move down for a fuller voice.
          - group "Interact with Pocket Theremin" [active] [ref=e33]:
            - text: high pitch ← pitch hand → low pitch
            - generic [ref=e34]: Touch and glide.
            - generic "plot output"
            - text: near volume loop · mute
          - paragraph [ref=e35]: Plots are awake
          - generic [ref=e36]:
            - button "lull body" [ref=e37] [cursor=pointer]
            - button "Finish body" [ref=e38] [cursor=pointer]
      - navigation "Your plots" [ref=e39]:
        - button "Tutorial" [ref=e40] [cursor=pointer]
        - button "Pocket Theremin" [pressed] [ref=e41] [cursor=pointer]
        - button "Memory Lantern" [ref=e42] [cursor=pointer]
        - button "+ Plots" [ref=e43] [cursor=pointer]
    - contentinfo "Current activity" [ref=e44]:
      - button "Pocket Theremin" [ref=e45] [cursor=pointer]
      - button "13 Gears · 13 Cords Look inside ↗" [ref=e46] [cursor=pointer]:
        - generic [ref=e47]: 13 Gears · 13 Cords
        - generic [ref=e48]: Look inside ↗
      - button "Playing" [ref=e49] [cursor=pointer]
    - status [ref=e52]: 3 Plots in Roseau.
```

# Test source

```ts
  48  |   await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  49  |   await page.reload();
  50  |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  51  |   expect((await current(page)).body_id).toBe(identity);
  52  | });
  53  | 
  54  | test("Use installs into the same body; repeated Use preserves Play and removal survives reload", async ({ page }, testInfo) => {
  55  |   await page.setViewportSize({ width: 1280, height: 1000 });
  56  |   await birth(page);
  57  |   const initial = await current(page);
  58  |   await openLibrary(page);
  59  |   await expect(card(page, "Memory Lantern")).toContainText("In your body");
  60  |   await page.screenshot({ path: testInfo.outputPath("workspace-plot-library.png"), fullPage: true });
  61  |   await page.getByRole("textbox", { name: "Find a plot", exact: true }).fill("desk");
  62  |   await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  63  |   await expect(page.locator("#surface-title")).toHaveText("Desk Telegraph");
  64  |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  65  |   const installed = await current(page);
  66  |   expect(installed.body_id).toBe(initial.body_id);
  67  |   expect(installed.here_part_id).toBe(initial.here_part_id);
  68  |   expect(installed.workload_revision).toBe(1);
  69  |   expect(installed.active_play_id).not.toBe(initial.active_play_id);
  70  |   expect(installed.initial_plots).toHaveLength(3);
  71  |   const tutorial = page.locator('[data-body-tutorial]');
  72  |   await expect(tutorial).toContainText("Invite another host");
  73  |   await tutorial.getByRole("button", { name: "Invite another host", exact: true }).click();
  74  |   await expect(page.getByRole("heading", { name: "parts and hosts", exact: true })).toBeVisible();
  75  |   await page.getByRole("button", { name: "back to the surface", exact: true }).click();
  76  |   await page.locator("#plot-input").focus();
  77  |   await page.keyboard.type("hello");
  78  |   await page.keyboard.press("Enter");
  79  |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("hello");
  80  |   await page.keyboard.type("again");
  81  |   await page.keyboard.press("Enter");
  82  |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("again");
  83  |   await openLibrary(page);
  84  |   await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  85  |   expect((await current(page)).active_play_id).toBe(installed.active_play_id);
  86  |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("again");
  87  |   await page.screenshot({ path: testInfo.outputPath("workspace-installed-plot.png"), fullPage: true });
  88  |   await openLibrary(page);
  89  |   await card(page, "Desk Telegraph").getByRole("button", { name: "Remove", exact: true }).click();
  90  |   await expect(card(page, "Desk Telegraph")).toContainText("Not in your body");
  91  |   await page.getByRole("button", { name: "back to the surface", exact: true }).click();
  92  |   await expect(page.locator("#surface-title")).toHaveText("Memory Lantern");
  93  |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  94  |   await page.locator("#plot-input").focus();
  95  |   await page.keyboard.press("r");
  96  |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("r");
  97  |   await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  98  |   await page.reload();
  99  |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  100 |   const restored = await current(page);
  101 |   expect(restored.body_id).toBe(initial.body_id);
  102 |   expect(restored.workload_revision).toBe(2);
  103 |   expect(restored.initial_plots).toEqual(initial.initial_plots);
  104 | });
  105 | 
  106 | test("Pocket Theremin plays continuous two-axis PCM on the browser host", async ({ page }) => {
  107 |   await page.addInitScript(() => {
  108 |     const NativeAudioContext = window.AudioContext;
  109 |     globalThis.__thereminAudio = { blocks: [], starts: [] };
  110 |     window.AudioContext = class extends NativeAudioContext {
  111 |       createBuffer(channels, frames, sampleRate) {
  112 |         globalThis.__thereminAudio.blocks.push({ channels, frames, sampleRate });
  113 |         return super.createBuffer(channels, frames, sampleRate);
  114 |       }
  115 |       createBufferSource() {
  116 |         const source = super.createBufferSource();
  117 |         const start = source.start.bind(source);
  118 |         source.start = when => {
  119 |           const samples = source.buffer.getChannelData(0);
  120 |           let peak = 0, crossings = 0;
  121 |           for (let index = 0; index < samples.length; index += 1) {
  122 |             peak = Math.max(peak, Math.abs(samples[index]));
  123 |             if (index > 0 && samples[index - 1] <= 0 && samples[index] > 0) crossings += 1;
  124 |           }
  125 |           globalThis.__thereminAudio.starts.push({ when, peak, crossings });
  126 |           return start(when);
  127 |         };
  128 |         return source;
  129 |       }
  130 |     };
  131 |   });
  132 |   await birth(page);
  133 |   await openLibrary(page);
  134 |   await page.getByRole("textbox", { name: "Find a plot", exact: true }).fill("theremin");
  135 |   await card(page, "Pocket Theremin").getByRole("button", { name: "Use", exact: true }).click();
  136 |   await expect(page.locator("#surface-title")).toHaveText("Pocket Theremin");
  137 |   const surface = page.locator("#plot-input[data-pocket-theremin]");
  138 |   await expect(surface).toBeVisible();
  139 |   // Raw mouse coordinates do not perform locator scrolling. The full two-axis
  140 |   // surface must be in view before measuring the points used by this gesture.
  141 |   await surface.scrollIntoViewIfNeeded();
  142 |   const bounds = await surface.boundingBox();
  143 |   const viewport = page.viewportSize();
  144 |   expect(bounds).not.toBeNull();
  145 |   expect(bounds.x).toBeGreaterThanOrEqual(0);
  146 |   expect(bounds.y).toBeGreaterThanOrEqual(0);
  147 |   expect(bounds.x + bounds.width).toBeLessThanOrEqual(viewport.width);
> 148 |   expect(bounds.y + bounds.height).toBeLessThanOrEqual(viewport.height);
      |                                    ^ Error: expect(received).toBeLessThanOrEqual(expected)
  149 |   await page.mouse.move(bounds.x + bounds.width * 0.15, bounds.y + bounds.height * 0.85);
  150 |   await page.mouse.down();
  151 |   await expect.poll(() => page.evaluate(() => globalThis.__thereminAudio.starts.length)).toBeGreaterThan(2);
  152 |   const highPitchEnd = await page.evaluate(() => globalThis.__thereminAudio.starts.length);
  153 |   await page.mouse.move(bounds.x + bounds.width * 0.98, bounds.y + bounds.height * 0.85, { steps: 8 });
  154 |   await expect.poll(() => page.evaluate(end => globalThis.__thereminAudio.starts.slice(end + 1).some(block => block.crossings <= 10), highPitchEnd)).toBe(true);
  155 |   const loudEnd = await page.evaluate(() => globalThis.__thereminAudio.starts.length);
  156 |   const loudPeak = await page.evaluate(({ start, end }) => Math.max(...globalThis.__thereminAudio.starts.slice(start + 1, end).map(block => block.peak)), { start: highPitchEnd, end: loudEnd });
  157 |   await page.mouse.move(bounds.x + bounds.width * 0.98, bounds.y + bounds.height * 0.15, { steps: 8 });
  158 |   await expect.poll(() => page.evaluate(({ end, threshold }) => globalThis.__thereminAudio.starts.slice(end + 1).some(block => block.peak < threshold), { end: loudEnd, threshold: loudPeak * 0.4 })).toBe(true);
  159 |   const audio = await page.evaluate(() => globalThis.__thereminAudio);
  160 |   expect(audio.blocks.every(block => block.channels === 1 && block.frames === 2000 && block.sampleRate === 48000)).toBe(true);
  161 |   expect(audio.starts[1].when).toBeGreaterThanOrEqual(audio.starts[0].when);
  162 |   expect(audio.starts.slice(1, highPitchEnd).some(block => block.crossings >= 20)).toBe(true);
  163 |   // 110 Hz spans 4.58 cycles in each 2,000-frame block, so phase-continuous
  164 |   // blocks contain either nine or ten zero crossings.
  165 |   expect(audio.starts.slice(highPitchEnd + 1, loudEnd).some(block => block.crossings <= 10)).toBe(true);
  166 |   const quietPeak = Math.min(...audio.starts.slice(loudEnd + 1).map(block => block.peak));
  167 |   expect(quietPeak).toBeLessThan(loudPeak * 0.4);
  168 |   await page.mouse.up();
  169 | });
  170 | 
  171 | test("the reviewed shelf reveals conversation Plots without pretending this browser can run them", async ({ page }) => {
  172 |   await birth(page);
  173 |   await openLibrary(page);
  174 |   await page.getByRole("textbox", { name: "Find a plot", exact: true }).fill("conversation");
  175 |   await expect(card(page, "Body Chat")).toContainText("Needs capability");
  176 |   await expect(card(page, "Body Chat")).toContainText("current body supervisor and an admitted model realization");
  177 |   await expect(card(page, "Live Conversation")).toContainText("Needs capability");
  178 |   await expect(card(page, "Live Conversation")).toContainText("live audio acquisition, streaming model generation, synthesis, and presentation");
  179 |   await expect(card(page, "Live Conversation")).toContainText("Text fallback: Body Chat");
  180 |   await expect(card(page, "Live Conversation")).toContainText("The fallback still needs capability");
  181 |   await expect(card(page, "House Spoken Response")).toContainText("Text fallback: House Conversation");
  182 |   await expect(card(page, "Body Chat").getByRole("button", { name: /Use — unavailable/u })).toBeDisabled();
  183 |   await expect(card(page, "Live Conversation").getByRole("button", { name: /Use — unavailable/u })).toBeDisabled();
  184 |   await expect(page.getByText("4 Plots", { exact: true })).toBeVisible();
  185 | });
  186 | 
  187 | test("window focus loss retires the pending Play and lets the same body wake again", async ({ page }) => {
  188 |   await birth(page);
  189 |   const before = await current(page);
  190 |   await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  191 |   await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  192 |   await expect(page.locator("#surface-guidance")).toContainText("lost focus");
  193 |   expect((await current(page)).body_id).toBe(before.body_id);
  194 |   expect((await current(page)).active_play_id).toBeUndefined();
  195 |   await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  196 |   await page.getByRole("button", { name: "wake body", exact: true }).click();
  197 |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  198 |   expect((await current(page)).body_id).toBe(before.body_id);
  199 |   expect((await current(page)).active_play_id).not.toBe(before.active_play_id);
  200 | });
  201 | 
  202 | test("removing the final Plot retains an empty Body that can acquire Plots again", async ({ page }, testInfo) => {
  203 |   await page.setViewportSize({ width: 390, height: 844 });
  204 |   await birth(page);
  205 |   const initial = await current(page);
  206 |   await openLibrary(page);
  207 |   await card(page, "Memory Lantern").getByRole("button", { name: "Remove", exact: true }).click();
  208 |   await expect(card(page, "Memory Lantern")).toContainText("Not in your body");
  209 |   await card(page, "Tutorial").getByRole("button", { name: "Remove", exact: true }).click();
  210 |   await expect(card(page, "Tutorial")).toContainText("Not in your body");
  211 |   await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  212 |   expect((await current(page)).initial_plots).toHaveLength(0);
  213 |   expect((await current(page)).active_play_id).toBeUndefined();
  214 |   expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  215 |   await page.screenshot({ path: testInfo.outputPath("workspace-empty-library-narrow.png"), fullPage: true });
  216 |   await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  217 |   await page.reload();
  218 |   await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  219 |   expect((await current(page)).body_id).toBe(initial.body_id);
  220 |   await openLibrary(page);
  221 |   await card(page, "Memory Lantern").getByRole("button", { name: "Use", exact: true }).click();
  222 |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  223 |   await expect(page.getByRole("button", { name: "lull body", exact: true })).toBeVisible();
  224 |   await page.keyboard.press("a");
  225 |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("a");
  226 |   expect((await current(page)).body_id).toBe(initial.body_id);
  227 | });
  228 | 
  229 | test("a failed workset save stops before replacement effects and reload recovers the saved workload", async ({ page }) => {
  230 |   await page.addInitScript(() => {
  231 |     const put = IDBObjectStore.prototype.put;
  232 |     globalThis.__failedWorksetWrites = 0;
  233 |     IDBObjectStore.prototype.put = function(record, ...args) {
  234 |       if (record.key === "body-session" && typeof record.value === "string") {
  235 |         const value = JSON.parse(record.value);
  236 |         if (value.evidence?.body.workload_revision > 0) {
  237 |           globalThis.__failedWorksetWrites++;
  238 |           throw new DOMException("Injected workset storage exhaustion", "QuotaExceededError");
  239 |         }
  240 |       }
  241 |       return put.call(this, record, ...args);
  242 |     };
  243 |   });
  244 |   await birth(page);
  245 |   const initial = await current(page);
  246 |   await openLibrary(page);
  247 |   await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  248 |   await expect(page.locator("[data-play-state]")).toHaveText("Stopped");
```