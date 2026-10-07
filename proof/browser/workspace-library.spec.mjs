import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./static-product-server.mjs";

let entrance, pageErrors;
test.beforeEach(async ({ page }) => { pageErrors = []; page.on("pageerror", error => pageErrors.push(error.message)); entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) console.error((await page.locator('body').innerText()).slice(0, 6000));
  entrance?.child.kill(); expect(pageErrors).toEqual([]);
});

const current = page => page.evaluate(() => globalThis.__conduitWorkspace.current());
const card = (page, title) => page.locator('[data-application-key^="library-plot-"]').filter({
  hasText: new RegExp(`^${title.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&")}`),
});
const openLibrary = page => page.getByRole("button", { name: "+ Plots", exact: true }).click();
async function birth(page) {
  await page.goto(entrance.url);
  const chime = page.getByRole("checkbox", { name: "Startup Chime", exact: true });
  await chime.uncheck();
  await page.getByLabel("Friendly Body name", { exact: true }).fill("Roseau");
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
}
test("five ordinary plots start together beyond the old aggregate placement ceiling", async ({ page }) => {
  await page.goto(entrance.url);
  for (const title of ["Button Across the Room", "Firefly Choir"]) {
    await page.getByRole("checkbox", { name: title, exact: true }).check();
  }
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const state = await current(page);
  expect(state.initial_plots).toHaveLength(5);
  expect(state.refusal).toBeUndefined();
});

test("repeated Wake and Lull compacts retained evidence instead of exhausting lifecycle Signs", async ({ page }) => {
  await birth(page);
  const identity = (await current(page)).body_id;
  for (let index = 0; index < 12; index++) {
    await page.getByRole("button", { name: "lull body", exact: true }).click();
    await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
    await page.getByRole("button", { name: "wake body", exact: true }).click();
    await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  }
  expect((await current(page)).body_id).toBe(identity);
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  expect((await current(page)).body_id).toBe(identity);
});

test("Use installs into the same body; repeated Use preserves Play and removal survives reload", async ({ page }, testInfo) => {
  // This journey includes three workset changes, interactive execution and reload.
  // Two retained Candidate traces passed preceding assertions but exhausted the
  // default 20-second aggregate budget during the final lifecycle/reload step.
  test.setTimeout(45_000);
  await page.setViewportSize({ width: 1280, height: 1000 });
  await birth(page);
  const initial = await current(page);
  await openLibrary(page);
  await expect(card(page, "Memory Lantern")).toContainText("In your body");
  await page.screenshot({ path: testInfo.outputPath("workspace-plot-library.png"), fullPage: true });
  await page.getByRole("textbox", { name: "Find a plot", exact: true }).fill("desk");
  await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("#surface-title")).toHaveText("Desk Telegraph");
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const installed = await current(page);
  expect(installed.body_id).toBe(initial.body_id);
  expect(installed.here_part_id).toBe(initial.here_part_id);
  expect(installed.workload_revision).toBe(1);
  expect(installed.active_play_id).not.toBe(initial.active_play_id);
  expect(installed.initial_plots).toHaveLength(3);
  const tutorial = page.locator('[data-body-tutorial]');
  await expect(tutorial).toContainText("Invite another host");
  await tutorial.getByRole("button", { name: "Invite another host", exact: true }).click();
  await expect(page.getByRole("heading", { name: "parts and hosts", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "back to the surface", exact: true }).click();
  await page.locator("#plot-input").focus();
  await page.keyboard.type("hello");
  await page.keyboard.press("Enter");
  await expect(page.locator("[data-plot-output] output:visible")).toHaveText("hello");
  await page.keyboard.type("again");
  await page.keyboard.press("Enter");
  await expect(page.locator("[data-plot-output] output:visible")).toHaveText("again");
  await openLibrary(page);
  await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  expect((await current(page)).active_play_id).toBe(installed.active_play_id);
  await expect(page.locator("[data-plot-output] output:visible")).toHaveText("again");
  await page.screenshot({ path: testInfo.outputPath("workspace-installed-plot.png"), fullPage: true });
  await openLibrary(page);
  await card(page, "Desk Telegraph").getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card(page, "Desk Telegraph")).toContainText("Not in your body");
  await page.getByRole("button", { name: "back to the surface", exact: true }).click();
  await expect(page.locator("#surface-title")).toHaveText("Memory Lantern");
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await page.locator("#plot-input").focus();
  await page.keyboard.press("r");
  await expect(page.locator("[data-plot-output] output:visible")).toHaveText("r");
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const restored = await current(page);
  expect(restored.body_id).toBe(initial.body_id);
  expect(restored.workload_revision).toBe(2);
  expect(restored.initial_plots).toEqual(initial.initial_plots);
});

test("Pocket Theremin plays continuous two-axis PCM on the browser host", async ({ page }) => {
  await page.addInitScript(() => {
    const NativeAudioContext = window.AudioContext;
    globalThis.__thereminAudio = { blocks: [], starts: [] };
    window.AudioContext = class extends NativeAudioContext {
      createBuffer(channels, frames, sampleRate) {
        globalThis.__thereminAudio.blocks.push({ channels, frames, sampleRate });
        return super.createBuffer(channels, frames, sampleRate);
      }
      createBufferSource() {
        const source = super.createBufferSource();
        const start = source.start.bind(source);
        source.start = when => {
          const samples = source.buffer.getChannelData(0);
          let peak = 0, crossings = 0;
          for (let index = 0; index < samples.length; index += 1) {
            peak = Math.max(peak, Math.abs(samples[index]));
            if (index > 0 && samples[index - 1] <= 0 && samples[index] > 0) crossings += 1;
          }
          globalThis.__thereminAudio.starts.push({ when, peak, crossings });
          return start(when);
        };
        return source;
      }
    };
  });
  await birth(page);
  await openLibrary(page);
  await page.getByRole("textbox", { name: "Find a plot", exact: true }).fill("theremin");
  await card(page, "Pocket Theremin").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("#surface-title")).toHaveText("Pocket Theremin");
  const surface = page.locator("#plot-input[data-pocket-theremin]");
  await expect(surface).toBeVisible();
  // Raw mouse coordinates do not perform locator scrolling. Bring the surface
  // into view and check every gesture extreme before starting pointer input.
  await surface.scrollIntoViewIfNeeded();
  const bounds = await surface.boundingBox();
  const viewport = page.viewportSize();
  expect(bounds).not.toBeNull();
  expect(bounds.x + bounds.width * 0.15).toBeGreaterThanOrEqual(0);
  expect(bounds.y + bounds.height * 0.15).toBeGreaterThanOrEqual(0);
  expect(bounds.x + bounds.width * 0.98).toBeLessThan(viewport.width);
  expect(bounds.y + bounds.height * 0.85).toBeLessThan(viewport.height);
  await page.mouse.move(bounds.x + bounds.width * 0.15, bounds.y + bounds.height * 0.85);
  await page.mouse.down();
  await expect.poll(() => page.evaluate(() => globalThis.__thereminAudio.starts.length)).toBeGreaterThan(2);
  const highPitchEnd = await page.evaluate(() => globalThis.__thereminAudio.starts.length);
  await page.mouse.move(bounds.x + bounds.width * 0.98, bounds.y + bounds.height * 0.85, { steps: 8 });
  await expect.poll(() => page.evaluate(end => globalThis.__thereminAudio.starts.slice(end + 1).some(block => block.crossings <= 10), highPitchEnd)).toBe(true);
  const loudEnd = await page.evaluate(() => globalThis.__thereminAudio.starts.length);
  const loudPeak = await page.evaluate(({ start, end }) => Math.max(...globalThis.__thereminAudio.starts.slice(start + 1, end).map(block => block.peak)), { start: highPitchEnd, end: loudEnd });
  await page.mouse.move(bounds.x + bounds.width * 0.98, bounds.y + bounds.height * 0.15, { steps: 8 });
  await expect.poll(() => page.evaluate(({ end, threshold }) => globalThis.__thereminAudio.starts.slice(end + 1).some(block => block.peak < threshold), { end: loudEnd, threshold: loudPeak * 0.4 })).toBe(true);
  const audio = await page.evaluate(() => globalThis.__thereminAudio);
  expect(audio.blocks.every(block => block.channels === 1 && block.frames === 2000 && block.sampleRate === 48000)).toBe(true);
  expect(audio.starts[1].when).toBeGreaterThanOrEqual(audio.starts[0].when);
  expect(audio.starts.slice(1, highPitchEnd).some(block => block.crossings >= 20)).toBe(true);
  // 110 Hz spans 4.58 cycles in each 2,000-frame block, so phase-continuous
  // blocks contain either nine or ten zero crossings.
  expect(audio.starts.slice(highPitchEnd + 1, loudEnd).some(block => block.crossings <= 10)).toBe(true);
  const quietPeak = Math.min(...audio.starts.slice(loudEnd + 1).map(block => block.peak));
  expect(quietPeak).toBeLessThan(loudPeak * 0.4);
  await page.mouse.up();
});

test("the reviewed shelf reveals conversation Plots without pretending this browser can run them", async ({ page }) => {
  await birth(page);
  await openLibrary(page);
  await page.getByRole("textbox", { name: "Find a plot", exact: true }).fill("conversation");
  await expect(card(page, "Body Chat")).toContainText("Needs capability");
  await expect(card(page, "Body Chat")).toContainText("current body supervisor and an admitted model realization");
  await expect(card(page, "Live Conversation")).toContainText("Needs capability");
  await expect(card(page, "Live Conversation")).toContainText("live audio acquisition, streaming model generation, synthesis, and presentation");
  await expect(card(page, "Live Conversation")).toContainText("Text fallback: Body Chat");
  await expect(card(page, "Live Conversation")).toContainText("The fallback still needs capability");
  await expect(card(page, "House Spoken Response")).toContainText("Text fallback: House Conversation");
  await expect(card(page, "Body Chat").getByRole("button", { name: /Use — unavailable/u })).toBeDisabled();
  await expect(card(page, "Live Conversation").getByRole("button", { name: /Use — unavailable/u })).toBeDisabled();
  await expect(page.getByText("4 Plots", { exact: true })).toBeVisible();
});

test("window focus loss retires the pending Play and lets the same body wake again", async ({ page }) => {
  await birth(page);
  const before = await current(page);
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  await expect(page.locator("#surface-guidance")).toContainText("lost focus");
  expect((await current(page)).body_id).toBe(before.body_id);
  expect((await current(page)).active_play_id).toBeUndefined();
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  expect((await current(page)).body_id).toBe(before.body_id);
  expect((await current(page)).active_play_id).not.toBe(before.active_play_id);
});

test("removing the final Plot retains an empty Body that can acquire Plots again", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await birth(page);
  const initial = await current(page);
  await openLibrary(page);
  await card(page, "Memory Lantern").getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card(page, "Memory Lantern")).toContainText("Not in your body");
  await card(page, "Tutorial").getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card(page, "Tutorial")).toContainText("Not in your body");
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  expect((await current(page)).initial_plots).toHaveLength(0);
  expect((await current(page)).active_play_id).toBeUndefined();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("workspace-empty-library-narrow.png"), fullPage: true });
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  expect((await current(page)).body_id).toBe(initial.body_id);
  await openLibrary(page);
  await card(page, "Memory Lantern").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await expect(page.getByRole("button", { name: "lull body", exact: true })).toBeVisible();
  await page.keyboard.press("a");
  await expect(page.locator("[data-plot-output] output:visible")).toHaveText("a");
  expect((await current(page)).body_id).toBe(initial.body_id);
});

test("a failed workset save stops before replacement effects and reload recovers the saved workload", async ({ page }) => {
  await page.addInitScript(() => {
    const put = IDBObjectStore.prototype.put;
    globalThis.__failedWorksetWrites = 0;
    IDBObjectStore.prototype.put = function(record, ...args) {
      if (record.key === "body-session" && typeof record.value === "string") {
        const value = JSON.parse(record.value);
        if (value.evidence?.body.workload_revision > 0) {
          globalThis.__failedWorksetWrites++;
          throw new DOMException("Injected workset storage exhaustion", "QuotaExceededError");
        }
      }
      return put.call(this, record, ...args);
    };
  });
  await birth(page);
  const initial = await current(page);
  await openLibrary(page);
  await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Stopped");
  const failed = await page.evaluate(() => ({ current: globalThis.__conduitWorkspace.current(), state: globalThis.__conduitWorkspace.state(), writes: globalThis.__failedWorksetWrites }));
  expect(failed.writes).toBe(1);
  expect(failed.current.active_play_id).toBeUndefined();
  expect(failed.state.terminal.disposition).toBe("cancelled");
  expect(failed.state.terminal.active_play_id).toBe(initial.active_play_id);
  await expect(page.locator("[data-plot-output] output")).toHaveCount(0);
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const restored = await current(page);
  expect(restored.body_id).toBe(initial.body_id);
  expect(restored.workload_revision).toBe(0);
  expect(restored.initial_plots).toEqual(initial.initial_plots);
});

async function reviewedPlot(page, name) {
  const source = await (await page.request.get(new URL('plots/initial-body.conduit', entrance.url).href)).text();
  return page.evaluate(({ source, name }) => {
    const api = globalThis.__conduitWorkspace.host.runtime;
    const bytes = new TextEncoder().encode(source);
    const pointer = api.conduit_creche_input_ptr();
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
    if (api.conduit_creche_reviewed_inventory(bytes.length) < 0) throw new Error('Inventory refused');
    const inventory = JSON.parse(new TextDecoder().decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len())));
    return inventory.plots.find(plot => plot.name === name);
  }, { source, name });
}
function handoffUrl(plot) {
  const url = new URL(entrance.url);
  url.search = new URLSearchParams({ plot: plot.name, source_document_id: plot.source_document_id, checked_plot_id: plot.checked_plot_id });
  return url.href;
}

test('a Gallery handoff installs in the retained body and cannot reinstall a later removed Plot on reload', async ({ page }) => {
  await birth(page);
  const initial = await current(page);
  const plot = await reviewedPlot(page, 'desk_telegraph');
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.goto(handoffUrl(plot));
  await expect(page.locator('#surface-title')).toHaveText('Desk Telegraph');
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  expect((await current(page)).body_id).toBe(initial.body_id);
  expect((await current(page)).initial_plots).toHaveLength(3);
  expect(new URL(page.url()).search).toBe('');
  await openLibrary(page);
  await card(page, 'Desk Telegraph').getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(card(page, 'Desk Telegraph')).toContainText('Not in your body');
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  expect((await current(page)).initial_plots).toEqual(initial.initial_plots);
});

test('a new Gallery arrival selects the plot in the actual Crèche; a stale handoff leaves the body usable', async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.getByRole('heading', { name: 'A body of your own' })).toBeVisible();
  const plot = await reviewedPlot(page, 'desk_telegraph');
  await page.goto(handoffUrl(plot));
  await expect(page.getByRole('checkbox', { name: 'Desk Telegraph', exact: true })).toBeChecked();
  await page.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await page.getByRole('button', { name: 'wake body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await expect(page.locator('#surface-title')).toHaveText('Desk Telegraph');
  const initial = await current(page);
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.goto(handoffUrl({ ...plot, checked_plot_id: 'checked/stale' }));
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await expect(page.locator('[data-workspace-notice]')).toContainText('checked identity has changed');
  expect((await current(page)).body_id).toBe(initial.body_id);
  expect((await current(page)).initial_plots).toEqual(initial.initial_plots);
});

for (const [title, kind] of [['Firefly Choir', 'pulse'], ['Night Radio', 'text'], ['Secret Knock', 'button']]) {
  test(`Gallery Plot ${title} is resident and repeatedly usable in the body`, async ({ page }, testInfo) => {
    await page.goto(entrance.url);
    await page.getByRole('checkbox', { name: 'Memory Lantern', exact: true }).uncheck();
    await page.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
    await page.getByRole('checkbox', { name: 'Tutorial', exact: true }).uncheck();
    await page.getByRole('checkbox', { name: title, exact: true }).check();
    await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
    await page.getByRole('button', { name: 'wake body', exact: true }).click();
    await expect(page.locator('[data-play-state]')).toHaveText('Playing');
    const identity = await current(page);
    const surface = page.locator('#plot-input');
    const output = page.locator('[data-plot-output] output:visible');
    if (kind === 'pulse') {
      await expect.poll(async () => Number((await output.filter({ hasText: /^pulse / }).textContent())?.match(/^pulse (\d+) ·/)?.[1] ?? 0)).toBeGreaterThanOrEqual(5);
    } else if (kind === 'text') {
      await page.keyboard.type('first'); await page.keyboard.press('Enter');
      await expect(output).toContainText('first');
      await page.keyboard.type('second'); await page.keyboard.press('Enter');
      await expect(output).toContainText('second');
    } else {
      await page.locator('[data-plot-output] output').evaluate(element => {
        element.dataset.presentationCount = '0';
        new MutationObserver(() => element.dataset.presentationCount = String(Number(element.dataset.presentationCount) + 1))
          .observe(element, { childList: true, characterData: true, subtree: true });
      });
      await surface.hover();
      try {
        for (const transitions of [['down', 'up', 'down', 'up', 'down'], ['up', 'down', 'up', 'down', 'up', 'down']]) {
          for (const transition of transitions) await page.mouse[transition]();
          await expect(output).toContainText('matched:');
          await expect(output).toContainText('score_millionths:');
          await expect(output).toHaveAttribute('data-presentation-count', transitions.length === 5 ? '1' : '2');
        }
      } finally { await page.mouse.up(); }
    }
    expect((await current(page)).active_play_id).toBe(identity.active_play_id);
    if (kind === 'button') await page.screenshot({ path: testInfo.outputPath(`workspace-${kind}-plot.png`), fullPage: true });
    await page.getByRole('button', { name: 'lull body', exact: true }).click();
    await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
  });
}
