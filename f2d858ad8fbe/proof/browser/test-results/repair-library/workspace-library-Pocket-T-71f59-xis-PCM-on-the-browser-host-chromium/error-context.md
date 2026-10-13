# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: workspace-library.spec.mjs >> Pocket Theremin plays continuous two-axis PCM on the browser host
- Location: proof/browser/workspace-library.spec.mjs:113:1

# Error details

```
Error: expect(received).toBeGreaterThan(expected)

Expected: > 2
Received:   0

Call Log:
- Timeout 5000ms exceeded while waiting on the predicate
```

# Page snapshot

```yaml
- generic [active] [ref=e1]:
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
        - text: lulled
        - generic [ref=e17]:
          - button "parts / hosts" [ref=e18] [cursor=pointer]
          - button "Invite another host" [ref=e19] [cursor=pointer]
    - group [ref=e20]:
      - generic "Learn through this body" [ref=e21] [cursor=pointer]
      - generic [ref=e23]:
        - text: Retained rest is not completion
        - status [ref=e24]: "Tutorial · lull — The body is lulled: its identity, Plots, and biography remain."
        - button "Wake the retained body" [ref=e25] [cursor=pointer]
        - status [ref=e26]: "Lifecycle evidence — Not yet fulfilled. Additional lifecycle evidence remains: Repair a real failed Wake; Admit another host. You can keep exploring."
    - main [ref=e27]:
      - region [ref=e28]:
        - generic [ref=e29]:
          - heading "Pocket Theremin" [level=2] [ref=e30]
          - paragraph [ref=e31]: "browser Body admission refused (-402): Plan exceeds the prepared browser storage/memory reservation"
          - group "Interact with Pocket Theremin" [disabled] [ref=e32]:
            - text: high pitch ← pitch hand → low pitch
            - generic "plot output"
            - text: near volume loop · mute
          - paragraph [ref=e33]: "browser Body admission refused (-402): Plan exceeds the prepared browser storage/memory reservation"
          - generic [ref=e34]:
            - button "wake body" [ref=e35] [cursor=pointer]
            - button "lull body" [disabled] [ref=e36]
            - button "Finish body" [disabled] [ref=e37]
      - navigation "Your plots" [ref=e38]:
        - button "Tutorial" [ref=e39] [cursor=pointer]
        - button "Pocket Theremin" [pressed] [ref=e40] [cursor=pointer]
        - button "Memory Lantern" [ref=e41] [cursor=pointer]
        - button "+ Plots" [ref=e42] [cursor=pointer]
    - contentinfo "Current activity" [ref=e43]:
      - button "Pocket Theremin" [ref=e44] [cursor=pointer]
      - button "Not yet planned Look inside ↗" [ref=e45] [cursor=pointer]:
        - generic [ref=e46]: Not yet planned
        - generic [ref=e47]: Look inside ↗
      - button "Refused" [ref=e48] [cursor=pointer]
    - status [ref=e51]: 3 Plots in Roseau.
```

# Test source

```ts
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
> 158 |   await expect.poll(() => page.evaluate(() => globalThis.__thereminAudio.starts.length)).toBeGreaterThan(2);
      |                                                                                          ^ Error: expect(received).toBeGreaterThan(expected)
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
  198 |   await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  199 |   await expect(page.locator("#surface-guidance")).toContainText("lost focus");
  200 |   expect((await current(page)).body_id).toBe(before.body_id);
  201 |   expect((await current(page)).active_play_id).toBeUndefined();
  202 |   await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  203 |   await page.getByRole("button", { name: "wake body", exact: true }).click();
  204 |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  205 |   expect((await current(page)).body_id).toBe(before.body_id);
  206 |   expect((await current(page)).active_play_id).not.toBe(before.active_play_id);
  207 | });
  208 | 
  209 | test("removing the final Plot retains an empty Body that can acquire Plots again", async ({ page }, testInfo) => {
  210 |   // This full lifecycle includes two library removals, a durable reload, and
  211 |   // a fresh Play. CI traces show all assertions can pass after the shared
  212 |   // 20-second suite deadline; give this multi-stage journey its own bound.
  213 |   test.setTimeout(30_000);
  214 |   await page.setViewportSize({ width: 390, height: 844 });
  215 |   await birth(page);
  216 |   const initial = await current(page);
  217 |   await openLibrary(page);
  218 |   await card(page, "Memory Lantern").getByRole("button", { name: "Remove", exact: true }).click();
  219 |   await expect(card(page, "Memory Lantern")).toContainText("Not in your body");
  220 |   await card(page, "Tutorial").getByRole("button", { name: "Remove", exact: true }).click();
  221 |   await expect(card(page, "Tutorial")).toContainText("Not in your body");
  222 |   await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  223 |   expect((await current(page)).initial_plots).toHaveLength(0);
  224 |   expect((await current(page)).active_play_id).toBeUndefined();
  225 |   expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  226 |   await page.screenshot({ path: testInfo.outputPath("workspace-empty-library-narrow.png"), fullPage: true });
  227 |   await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  228 |   await page.reload();
  229 |   await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  230 |   expect((await current(page)).body_id).toBe(initial.body_id);
  231 |   await openLibrary(page);
  232 |   await card(page, "Memory Lantern").getByRole("button", { name: "Use", exact: true }).click();
  233 |   await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  234 |   await expect(page.getByRole("button", { name: "lull body", exact: true })).toBeVisible();
  235 |   await page.keyboard.press("a");
  236 |   await expect(page.locator("[data-plot-output] output:visible")).toHaveText("a");
  237 |   expect((await current(page)).body_id).toBe(initial.body_id);
  238 | });
  239 | 
  240 | test("a failed workset save stops before replacement effects and reload recovers the saved workload", async ({ page }) => {
  241 |   await page.addInitScript(() => {
  242 |     const put = IDBObjectStore.prototype.put;
  243 |     globalThis.__failedWorksetWrites = 0;
  244 |     IDBObjectStore.prototype.put = function(record, ...args) {
  245 |       if (record.key === "body-session" && typeof record.value === "string") {
  246 |         const value = JSON.parse(record.value);
  247 |         if (value.evidence?.body.workload_revision > 0) {
  248 |           globalThis.__failedWorksetWrites++;
  249 |           throw new DOMException("Injected workset storage exhaustion", "QuotaExceededError");
  250 |         }
  251 |       }
  252 |       return put.call(this, record, ...args);
  253 |     };
  254 |   });
  255 |   await birth(page);
  256 |   const initial = await current(page);
  257 |   await openLibrary(page);
  258 |   await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
```