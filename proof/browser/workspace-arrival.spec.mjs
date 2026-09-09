import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;
test.beforeEach(async () => { entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
test.afterEach(() => entrance?.child.kill());

test("Birth arrives in listening Forms; foreground changes and reload preserve the Body", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.goto(entrance.url);
  const birth = page.locator(".body-birth-runner");
  await expect(birth.getByRole("heading", { name: "A Body of your own" })).toBeVisible();
  await expect(birth.getByRole("checkbox", { name: "Memory Lantern", exact: true })).toBeChecked();
  await birth.getByLabel("Friendly Body name", { exact: true }).fill("Roseau");
  await birth.getByRole("checkbox", { name: "Desk Telegraph", exact: true }).check();
  await page.screenshot({ path: testInfo.outputPath("workspace-birth.png"), fullPage: true });
  await birth.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-body-name]")).toHaveText("Roseau");
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await expect(page.locator("[data-wake-body]")).toBeHidden();
  const identity = () => page.evaluate(() => globalThis.__conduitWorkspace.current());
  const first = await identity();
  await expect(page.getByRole("navigation", { name: "Your Forms" }).locator("[data-checked-form-id]")).toHaveCount(3);
  await page.getByRole("navigation", { name: "Your Forms" }).getByRole("button", { name: "Memory Lantern", exact: true }).click();
  const output = page.locator("[data-form-output] output:visible");
  await page.keyboard.press("h");
  await expect(output).toHaveText("h");
  await page.keyboard.press("i");
  await expect(output).toHaveText("hi");
  await page.getByRole("navigation", { name: "Your Forms" }).getByRole("button", { name: "Desk Telegraph", exact: true }).click();
  await page.keyboard.press("o");
  await page.keyboard.press("k");
  await page.keyboard.press("Enter");
  await expect(output).toHaveText("ok");
  await page.getByRole("navigation", { name: "Your Forms" }).getByRole("button", { name: "Memory Lantern", exact: true }).click();
  await expect(output).toHaveText("hi");
  await expect(page.locator("[data-flow-label]")).toHaveText("keyboard → keymap → edit → text");
  await page.keyboard.press("Backspace");
  await expect(output).toHaveText("h");
  expect((await identity()).active_play_id).toBe(first.active_play_id);
  expect((await identity()).plan_id).toBe(first.plan_id);
  await page.screenshot({ path: testInfo.outputPath("workspace-listening.png"), fullPage: true });

  await page.getByRole("navigation", { name: "Your Forms" }).getByRole("button", { name: "Desk Telegraph", exact: true }).click();
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await expect(page.locator("#surface-title")).toHaveText("Desk Telegraph");
  const restored = await identity();
  expect(restored.body_id).toBe(first.body_id);
  expect(restored.host_id).toBe(first.host_id);
  expect(restored.boot_id).not.toBe(first.boot_id);
  expect(restored.plan_id).not.toBe(first.plan_id);
  expect(restored.active_play_id).not.toBe(first.active_play_id);
  await page.keyboard.press("n");
  await page.keyboard.press("Enter");
  await expect(output).toHaveText("n");
  await page.screenshot({ path: testInfo.outputPath("workspace-returned.png"), fullPage: true });
  await page.getByRole("button", { name: "Lull Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  expect((await identity()).body_id).toBe(first.body_id);
});

test("a failed started-state save cancels the real Play before dispatching effects", async ({ page }) => {
  await observeRealAudio(page);
  await page.addInitScript(() => {
    const put = IDBObjectStore.prototype.put;
    globalThis.__failedBodyWrites = 0;
    IDBObjectStore.prototype.put = function(record, ...args) {
      if (record.key === "body-session" && typeof record.value === "string") {
        const value = JSON.parse(record.value);
        if (value.schema === "conduit.workspace/body@1" && value.evidence.wakes?.some(wake => wake.lifecycle === "Playing")) {
          globalThis.__failedBodyWrites += 1;
          throw new DOMException("Injected Body storage exhaustion", "QuotaExceededError");
        }
      }
      return put.call(this, record, ...args);
    };
  });
  await page.goto(entrance.url);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Stopped");
  await expect(page.locator("#surface-guidance")).toContainText("Your Body could not be saved");
  await expect(page.getByRole("button", { name: "Wake Body", exact: true })).toBeDisabled();
  await expect(page.locator("#form-input")).toHaveAttribute("aria-disabled", "true");
  const result = await page.evaluate(() => ({ state: globalThis.__conduitWorkspace.state(), writes: globalThis.__failedBodyWrites }));
  expect(result.writes).toBe(1);
  expect(result.state.terminal.disposition).toBe("cancelled");
  expect(result.state.refusal.code).toBe("QuotaExceededError");
  expect(result.state.terminal.active_play_id).toBe(result.state.play.active_play_id);
  expect(result.state.terminal.manifestation_completions).toBe(0);
  expect(await page.evaluate(() => globalThis.__cueAudio.starts.length)).toBe(0);
  await expect(page.locator("[data-form-output] output")).toHaveCount(0);
});

test("a second window cannot recover a Body while its first Host is alive", async ({ page, context }) => {
  await page.goto(entrance.url);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const other = await context.newPage();
  await other.goto(entrance.url);
  await expect(other.locator("[data-workspace-notice]")).toContainText("This Body is open in another window");
  await expect(other.locator("[data-workspace-surface]")).toBeHidden();
  await page.locator("#form-input").focus();
  await page.keyboard.press("a");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("a");
  await other.close();
});

test("Birth and the listening surface fit a narrow window", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(entrance.url);
  await expect(page.getByRole("heading", { name: "A Body of your own" })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await page.keyboard.press("h");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("h");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("workspace-narrow.png"), fullPage: true });
});

test("an intentionally empty Body remains lulled without inventing a Play", async ({ page }) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  await expect(page.getByRole("heading", { name: "No Forms installed", exact: true })).toBeVisible();
  await expect(page.locator("#form-input")).toBeHidden();
  await expect(page.getByRole("button", { name: "Wake Body", exact: true })).toBeHidden();
  const original = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(original.active_play_id).toBeUndefined();
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  const returned = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(returned.body_id).toBe(original.body_id);
  expect(returned.active_play_id).toBeUndefined();
});

async function observeRealAudio(page) {
  await page.addInitScript(() => {
    globalThis.__cueAudio = { starts: [], ended: 0 };
    const start = AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start = function(...args) {
      globalThis.__cueAudio.starts.push({ frames: this.buffer.length, sampleRate: this.buffer.sampleRate, channels: this.buffer.numberOfChannels, state: this.context.state });
      this.addEventListener('ended', () => globalThis.__cueAudio.ended++, { once: true });
      return start.apply(this, args);
    };
  });
}
async function liveEvidence(page) {
  await page.locator('[data-inspect="lifecycle"]').click();
  const result = JSON.parse(await page.locator('[data-inspection-content] pre').textContent());
  await page.locator('[data-close-inspection]').click();
  return result.active_observation;
}

test("a sound-only Body renders the original cue through real browser audio and settles idle", async ({ page }, testInfo) => {
  await observeRealAudio(page);
  await page.goto(entrance.url);
  await page.getByLabel('Friendly Body name', { exact: true }).fill('Chime');
  await page.getByRole('checkbox', { name: 'Memory Lantern', exact: true }).uncheck();
  await expect(page.getByRole('checkbox', { name: 'Startup Chime', exact: true })).toBeChecked();
  await page.screenshot({ path: testInfo.outputPath('chime-birth.png'), fullPage: true });
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Idle');
  const audio = await page.evaluate(() => globalThis.__cueAudio);
  expect(audio.starts).toEqual([{ frames: 57600, sampleRate: 48000, channels: 1, state: 'running' }]);
  expect(audio.ended).toBe(1);
  await expect(page.locator('#form-input')).toBeHidden();
  await expect(page.locator('[data-form-output] output')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Wake Body', exact: true })).toBeHidden();
  const evidence = await liveEvidence(page);
  expect(evidence.host_completions.records.map(record => record.disposition)).toEqual(['completed']);
  await page.screenshot({ path: testInfo.outputPath('chime-idle.png'), fullPage: true });
  await page.getByRole('button', { name: 'Lull Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
  await page.getByRole('button', { name: 'Wake Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Idle');
  expect(await page.evaluate(() => globalThis.__cueAudio.starts.length)).toBe(2);
});

test("first-wake audio stays silent after reload of the same Body with a fresh Boot", async ({ page }) => {
  await observeRealAudio(page);
  await page.goto(entrance.url);
  await page.getByRole('checkbox', { name: 'Memory Lantern', exact: true }).uncheck();
  await page.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
  await page.getByRole('checkbox', { name: 'First Wake Chime', exact: true }).check();
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Idle');
  expect(await page.evaluate(() => globalThis.__cueAudio.starts.length)).toBe(1);
  const first = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator('[data-play-state]')).toHaveText('Idle');
  const returned = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(returned.body_id).toBe(first.body_id);
  expect(returned.boot_id).not.toBe(first.boot_id);
  expect(returned.active_play_id).not.toBe(first.active_play_id);
  expect(await page.evaluate(() => globalThis.__cueAudio.starts.length)).toBe(0);
  expect((await liveEvidence(page)).host_completions.records).toEqual([]);
});

test("removing the default cue survives reload and a later first-wake installation stays silent", async ({ page }) => {
  await observeRealAudio(page);
  await page.goto(entrance.url);
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await expect.poll(() => page.evaluate(() => globalThis.__cueAudio.ended)).toBe(1);
  const original = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  const card = title => page.locator('[data-application-key^="library-form-"]').filter({ hasText: title });
  await page.getByRole('button', { name: '+ Forms', exact: true }).click();
  await card('Startup Chime').getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(card('Startup Chime')).toContainText('Not in your Body');
  await card('First Wake Chime').getByRole('button', { name: 'Use', exact: true }).click();
  await expect(page.locator('#surface-title')).toHaveText('First Wake Chime');
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  expect(await page.evaluate(() => globalThis.__cueAudio.starts.length)).toBe(1);
  expect((await liveEvidence(page)).host_completions.records).toEqual([]);

  await page.getByRole('button', { name: 'Lull Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
  await page.getByRole('button', { name: 'Wake Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  expect(await page.evaluate(() => globalThis.__cueAudio.starts.length)).toBe(1);
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  const restored = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(restored.body_id).toBe(original.body_id);
  expect(restored.boot_id).not.toBe(original.boot_id);
  expect(await page.evaluate(() => globalThis.__cueAudio.starts.length)).toBe(0);
  expect((await liveEvidence(page)).host_completions.records).toEqual([]);
  await page.getByRole('button', { name: '+ Forms', exact: true }).click();
  await expect(card('Startup Chime')).toContainText('Not in your Body');
  await expect(card('First Wake Chime')).toContainText('In your Body');
  await page.getByRole('button', { name: 'Back to the surface', exact: true }).click();
  await page.getByRole('navigation', { name: 'Your Forms' }).getByRole('button', { name: 'Memory Lantern', exact: true }).click();
  await page.keyboard.press('a');
  await expect(page.locator('[data-form-output] output:visible')).toHaveText('a');
});

test("unavailable audio is omitted by default and an explicitly installed cue cannot stop other Forms", async ({ page }, testInfo) => {
  await page.addInitScript(() => { window.AudioContext = undefined; });
  await page.goto(entrance.url);
  await expect(page.getByRole('checkbox', { name: 'Startup Chime', exact: true })).not.toBeChecked();
  await page.getByRole('checkbox', { name: 'Startup Chime', exact: true }).check();
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await page.keyboard.press('a');
  await expect(page.locator('[data-form-output] output:visible')).toHaveText('a');
  const evidence = await liveEvidence(page);
  expect(evidence.host_completions.records.some(record => record.disposition === 'failed' && record.failure_code === 'host_operation_failed' && record.failure_detail === 1)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('chime-unavailable-body-listening.png'), fullPage: true });
});

test("a suspended real audio context reports denial while the Body keeps listening", async ({ page }) => {
  await page.addInitScript(() => {
    const AudioContext = window.AudioContext;
    window.AudioContext = class extends AudioContext {
      constructor(...args) { super(...args); this.suspend(); }
    };
  });
  await page.goto(entrance.url);
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await page.keyboard.press('b');
  await expect(page.locator('[data-form-output] output:visible')).toHaveText('b');
  const evidence = await liveEvidence(page);
  expect(evidence.host_completions.records.some(record => record.disposition === 'denied' && record.failure_code === 'host_operation_denied' && record.failure_detail === 2)).toBe(true);
});
