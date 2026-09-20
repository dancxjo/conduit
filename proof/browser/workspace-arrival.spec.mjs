import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;
test.beforeEach(async () => { entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
test.afterEach(() => entrance?.child.kill());

async function inventory(page) {
  const response = await page.request.get(new URL("forms/initial-body.conduit", entrance.url).href);
  expect(response.ok()).toBe(true);
  return response.json();
}

function selectedSource(bundled, names) {
  return bundled.forms
    .filter((form) => names.includes(form.entry ?? form.slug.replaceAll('-', '_')))
    .map((form) => form.source.trimEnd())
    .join("\n\n");
}

test("Birth hands off to a Lulled Body; Wake starts listening Forms and reload preserves the body", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.goto(entrance.url);
  const birth = page.locator(".body-birth-runner");
  await expect(birth.getByRole("heading", { name: "A body of your own" })).toBeVisible();
  await expect(birth.getByRole("checkbox", { name: "Memory Lantern", exact: true })).toBeChecked();
  await birth.getByLabel("Friendly Body name", { exact: true }).fill("Roseau");
  await birth.getByRole("checkbox", { name: "Desk Telegraph", exact: true }).check();
  await page.screenshot({ path: testInfo.outputPath("workspace-birth.png"), fullPage: true });
  await birth.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-body-name]")).toHaveText("Roseau");
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  await expect(page.locator("#surface-guidance")).toContainText("Wake it to start its forms");
  await expect(page.locator("#form-input")).toHaveAttribute("aria-disabled", "true");
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await expect(page.locator("[data-wake-body]")).toBeHidden();
  const identity = () => page.evaluate(() => globalThis.__conduitWorkspace.current());
  const first = await identity();
  await expect(page.getByRole("navigation", { name: "Your forms" }).locator("[data-checked-form-id]")).toHaveCount(4);
  await page.getByRole("navigation", { name: "Your forms" }).getByRole("button", { name: "Memory Lantern", exact: true }).click();
  const output = page.locator("[data-form-output] output:visible");
  await page.keyboard.press("h");
  await expect(output).toHaveText("h");
  await page.keyboard.press("i");
  await expect(output).toHaveText("hi");
  await page.getByRole("navigation", { name: "Your forms" }).getByRole("button", { name: "Desk Telegraph", exact: true }).click();
  await page.keyboard.press("o");
  await page.keyboard.press("k");
  await page.keyboard.press("Enter");
  await expect(output).toHaveText("ok");
  await page.getByRole("navigation", { name: "Your forms" }).getByRole("button", { name: "Memory Lantern", exact: true }).click();
  await expect(output).toHaveText("hi");
  await expect(page.locator("[data-flow-label]")).toHaveText("keyboard → keymap → edit → text");
  await page.keyboard.press("Backspace");
  await expect(output).toHaveText("h");
  expect((await identity()).active_play_id).toBe(first.active_play_id);
  expect((await identity()).plan_id).toBe(first.plan_id);
  await page.screenshot({ path: testInfo.outputPath("workspace-listening.png"), fullPage: true });

  await page.getByRole("navigation", { name: "Your forms" }).getByRole("button", { name: "Desk Telegraph", exact: true }).click();
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
  await page.getByRole("button", { name: "lull body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  expect((await identity()).body_id).toBe(first.body_id);
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  await expect(page.getByRole("button", { name: "wake body", exact: true })).toBeVisible();
  expect((await identity()).active_play_id).toBeUndefined();
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
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Stopped");
  await expect(page.locator("#surface-guidance")).toContainText("Your body could not be saved");
  await expect(page.getByRole("button", { name: "wake body", exact: true })).toBeDisabled();
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

test("a second window cannot recover a body while its first host is alive", async ({ page, context }) => {
  await page.goto(entrance.url);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const other = await context.newPage();
  await other.goto(entrance.url);
  await expect(other.locator("[data-workspace-notice]")).toContainText("This body is open in another window");
  await expect(other.locator("[data-workspace-surface]")).toBeHidden();
  await page.locator("#form-input").focus();
  await page.keyboard.press("a");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("a");
  await other.close();
});

test("Birth and the listening surface fit a narrow window", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(entrance.url);
  await expect(page.getByRole("heading", { name: "A body of your own" })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await page.keyboard.press("h");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("h");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("workspace-narrow.png"), fullPage: true });
});

test("an intentionally empty Body remains lulled without inventing a play", async ({ page }) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Tutorial", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  await expect(page.getByRole("heading", { name: "No Forms installed", exact: true })).toBeVisible();
  await expect(page.locator("#form-input")).toBeHidden();
  await expect(page.getByRole("button", { name: "wake body", exact: true })).toBeHidden();
  const original = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(original.active_play_id).toBeUndefined();
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  const returned = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(returned.body_id).toBe(original.body_id);
  expect(returned.active_play_id).toBeUndefined();
});

test("contextual tutorial follows the real Body instead of retaining lesson progress", async ({ page }) => {
  await page.goto(entrance.url);
  const tutorial = page.locator('[data-body-tutorial]');
  await expect(tutorial).toBeHidden();
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(tutorial).toContainText('Wake this body');
  await expect(tutorial).toContainText('Purpose · exact readiness');
  await expect(tutorial).toContainText('not ready · 5 exact obligation(s) remain');
  await expect(tutorial.getByRole('button', { name: 'Wake the retained body' })).toBeVisible();
  await page.getByRole('button', { name: 'wake body', exact: true }).click();
  await expect(tutorial).toContainText('finite Body may remain awake');
  await expect(tutorial).toContainText('not ready · 2 exact obligation(s) remain');
  await page.getByRole('button', { name: 'lull body', exact: true }).click();
  await expect(tutorial).toContainText('Retained rest is not completion');
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(tutorial).toContainText('Retained rest is not completion');
});

test("explicit Finish retires work, records provenance, and restores as read-only Fulfilled history", async ({ page }) => {
  await page.goto(entrance.url);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "Finish body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Fulfilled");
  await expect(page.locator("#surface-guidance")).toContainText("biography remains available");
  await expect(page.getByRole("button", { name: "wake body", exact: true })).toBeHidden();
  await expect(page.getByRole("button", { name: "Finish body", exact: true })).toBeHidden();
  const terminal = await page.evaluate(() => {
    const snapshot = globalThis.__conduitWorkspace.evidence();
    return { current: globalThis.__conduitWorkspace.current(), record: snapshot.evidence.records.at(-1) };
  });
  expect(terminal.current.state).toBe("FULFILLED");
  expect(terminal.record.kind.Fulfilled.attribution).toContain("operator/browser/");
  expect(terminal.record.kind.Fulfilled.settled_obligations[0].obligation_id).toBe("obligation/workspace-runtime-empty");

  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Fulfilled");
  await expect(page.getByRole("button", { name: "+ Forms", exact: true })).toHaveCount(0);
  await page.locator('[data-inspect="lifecycle"]').click();
  await expect(page.getByText("Exact lifecycle evidence", { exact: true })).toBeVisible();
  expect((await page.evaluate(() => globalThis.__conduitWorkspace.current())).state).toBe("FULFILLED");
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
  await page.getByRole('checkbox', { name: 'Tutorial', exact: true }).uncheck();
  await expect(page.getByRole('checkbox', { name: 'Startup Chime', exact: true })).toBeChecked();
  await page.screenshot({ path: testInfo.outputPath('chime-birth.png'), fullPage: true });
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await page.getByRole('button', { name: 'wake body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Idle');
  const audio = await page.evaluate(() => globalThis.__cueAudio);
  expect(audio.starts).toEqual([{ frames: 57600, sampleRate: 48000, channels: 1, state: 'running' }]);
  expect(audio.ended).toBe(1);
  await expect(page.locator('#form-input')).toBeHidden();
  await expect(page.locator('[data-form-output] output')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'wake body', exact: true })).toBeHidden();
  const evidence = await liveEvidence(page);
  expect(evidence.host_completions.records.map(record => record.disposition)).toEqual(['completed']);
  await page.screenshot({ path: testInfo.outputPath('chime-idle.png'), fullPage: true });
  await page.getByRole('button', { name: 'lull body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
  await page.getByRole('button', { name: 'wake body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Idle');
  expect(await page.evaluate(() => globalThis.__cueAudio.starts.length)).toBe(2);
});

test("first-wake audio stays silent after reload of the same body with a fresh Boot", async ({ page }) => {
  await observeRealAudio(page);
  await page.goto(entrance.url);
  await page.getByRole('checkbox', { name: 'Memory Lantern', exact: true }).uncheck();
  await page.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
  await page.getByRole('checkbox', { name: 'Tutorial', exact: true }).uncheck();
  await page.getByRole('checkbox', { name: 'First wake Chime', exact: true }).check();
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await page.getByRole('button', { name: 'wake body', exact: true }).click();
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
  await page.getByRole('checkbox', { name: 'Tutorial', exact: true }).uncheck();
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await page.getByRole('button', { name: 'wake body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await expect.poll(() => page.evaluate(() => globalThis.__cueAudio.ended)).toBe(1);
  const original = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  const card = title => page.locator('[data-application-key^="library-form-"]').filter({ hasText: title });
  await page.getByRole('button', { name: '+ Forms', exact: true }).click();
  await card('Startup Chime').getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(card('Startup Chime')).toContainText('Not in your body');
  await card('First wake Chime').getByRole('button', { name: 'Use', exact: true }).click();
  await expect(page.locator('#surface-title')).toHaveText('First wake Chime');
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  expect(await page.evaluate(() => globalThis.__cueAudio.starts.length)).toBe(1);
  expect((await liveEvidence(page)).host_completions.records).toEqual([]);

  await page.getByRole('button', { name: 'lull body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
  await page.getByRole('button', { name: 'wake body', exact: true }).click();
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
  await expect(card('Startup Chime')).toContainText('Not in your body');
  await expect(card('First wake Chime')).toContainText('In your body');
  await page.getByRole('button', { name: 'back to the surface', exact: true }).click();
  await page.getByRole('navigation', { name: 'Your forms' }).getByRole('button', { name: 'Memory Lantern', exact: true }).click();
  await page.keyboard.press('a');
  await expect(page.locator('[data-form-output] output:visible')).toHaveText('a');
});

test("unavailable audio is omitted by default and an explicitly installed cue cannot stop other Forms", async ({ page }, testInfo) => {
  await page.addInitScript(() => { window.AudioContext = undefined; });
  await page.goto(entrance.url);
  await expect(page.getByRole('checkbox', { name: 'Startup Chime', exact: true })).not.toBeChecked();
  await page.getByRole('checkbox', { name: 'Startup Chime', exact: true }).check();
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await page.getByRole('button', { name: 'wake body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await page.keyboard.press('a');
  await expect(page.locator('[data-form-output] output:visible')).toHaveText('a');
  const evidence = await liveEvidence(page);
  expect(evidence.host_completions.records.some(record => record.disposition === 'failed' && record.failure_code === 'host_operation_failed' && record.failure_detail === 1)).toBe(true);
  await page.screenshot({ path: testInfo.outputPath('chime-unavailable-body-listening.png'), fullPage: true });
});

test("a suspended real audio context reports denial while the body keeps listening", async ({ page }) => {
  await page.addInitScript(() => {
    const AudioContext = window.AudioContext;
    window.AudioContext = class extends AudioContext {
      constructor(...args) { super(...args); this.suspend(); }
    };
  });
  await page.goto(entrance.url);
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await page.getByRole('button', { name: 'wake body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await page.keyboard.press('b');
  await expect(page.locator('[data-form-output] output:visible')).toHaveText('b');
  const evidence = await liveEvidence(page);
  expect(evidence.host_completions.records.some(record => record.disposition === 'denied' && record.failure_code === 'host_operation_denied' && record.failure_detail === 2)).toBe(true);
});

test("Workspace Birth binds naming, search, exact selection, review, and receipt", async ({ page }) => {
  await page.goto(entrance.url);
  const birth = page.locator('.body-birth-runner');
  await expect(birth.getByRole('heading', { name: 'A body of your own', exact: true })).toBeVisible();

  const name = birth.getByLabel('Friendly Body name', { exact: true });
  const tradition = birth.getByLabel('Naming tradition', { exact: true });
  await expect(tradition.locator('option')).toHaveCount(24);
  await tradition.selectOption('roman');
  await expect(tradition.locator('option:checked')).toContainText('Roman');
  await name.fill('Juniper Signalhouse');
  await expect(name).toHaveValue('Juniper Signalhouse');

  const memory = birth.getByRole('checkbox', { name: 'Memory Lantern', exact: true });
  const desk = birth.getByRole('checkbox', { name: 'Desk Telegraph', exact: true });
  await memory.check();
  await birth.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
  await birth.getByRole('checkbox', { name: 'Tutorial', exact: true }).uncheck();
  const selected = birth.locator('[data-application-key="selected-forms"]');
  const initialCount = Number((await selected.textContent()).match(/\d+/u)?.[0]);
  expect(initialCount).toBeGreaterThan(0);

  const search = birth.getByLabel('Search Forms', { exact: true });
  await search.fill('no-such-form');
  await expect(birth.locator('[data-application-key="initial-forms"]')).toHaveText(
    'No Forms match your search. — Your selected forms are still included.',
  );
  await expect(selected).toHaveText(`Selected: ${initialCount}`);
  await search.fill('');
  await expect(memory).toBeChecked();
  await desk.check();
  await expect(selected).toHaveText(`Selected: ${initialCount + 1}`);
  await desk.uncheck();
  await expect(selected).toHaveText(`Selected: ${initialCount}`);
  await desk.check();

  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  const restored = page.locator('.body-birth-runner');
  await expect(restored.getByRole('checkbox', { name: 'Memory Lantern', exact: true })).toBeChecked();
  await expect(restored.getByRole('checkbox', { name: 'Desk Telegraph', exact: true })).toBeChecked();

  await restored.getByText('Details and source', { exact: true }).click();
  const source = restored.getByLabel('Selected Conduit Form source', { exact: true });
  const bundled = await inventory(page);
  const expectedSource = selectedSource(bundled, ['memory_lantern', 'desk_telegraph']);
  await expect(source).toHaveValue(expectedSource);
  await restored.getByRole('button', { name: 'Review workload', exact: true }).click();
  await expect(restored.locator('[data-application-key="review-basis"]')).toContainText(
    'current host OFFER(s); no permission or resource acquired; no Body Plan or Play created',
  );
  await expect(restored.locator('[data-application-key="birth-status"]')).toHaveText('Ready to birth with 2 Form(s).');

  await restored.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
  const receipt = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(receipt.state).toBe('LULLED');
  expect(receipt.workload_revision).toBe(0);
  expect(receipt.active_play_id).toBeUndefined();
  expect(receipt.initial_forms).toHaveLength(2);
  expect(receipt.initial_forms).toEqual([
    { source_document_id: expect.any(String), checked_form_id: expect.any(String) },
    { source_document_id: expect.any(String), checked_form_id: expect.any(String) },
  ]);
  expect(new Set(receipt.initial_forms.map(({ source_document_id }) => source_document_id)).size).toBe(2);
  expect(new Set(receipt.initial_forms.map(({ checked_form_id }) => checked_form_id)).size).toBe(2);
});

test('Workspace Birth serves the canonical reviewed inventory source without edits', async ({ page }) => {
  await page.goto(entrance.url);
  const bundled = await inventory(page);
  for (const { slug } of bundled.forms) {
    const canonical = readFileSync(new URL(`../../forms/${slug}/main.conduit`, import.meta.url), 'utf8');
    expect(bundled.forms.find(form => form.slug === slug)?.source).toBe(canonical);
  }

  const birth = page.locator('.body-birth-runner');
  await birth.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
  await birth.getByRole('checkbox', { name: 'Tutorial', exact: true }).uncheck();
  await birth.getByRole('checkbox', { name: 'Desk Telegraph', exact: true }).check();
  await birth.getByText('Details and source', { exact: true }).click();
  const source = birth.getByLabel('Selected Conduit Form source', { exact: true });
  await expect(source).toHaveValue(selectedSource(bundled, ['memory_lantern', 'desk_telegraph']));
  await expect(source).not.toHaveValue(/conduit\.creche\/reviewed-form-bundle/u);
});

test('Workspace Birth controls remain bounded and usable at a narrow width', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(entrance.url);
  const birth = page.locator('.body-birth-runner');
  await expect(birth.getByRole('heading', { name: 'A body of your own', exact: true })).toBeVisible();
  const [forms, name, sourceDetails, editor] = await Promise.all([
    birth.locator('[data-application-slot="birth-fields"] [data-application-key="initial-forms"]').boundingBox(),
    birth.getByLabel('Friendly Body name', { exact: true }).boundingBox(),
    birth.locator('.birth-presentation .birth-details').boundingBox(),
    birth.locator('.birth-presentation').boundingBox(),
  ]);
  for (const box of [forms, name, sourceDetails, editor]) expect(box).not.toBeNull();
  expect(name.y + name.height).toBeLessThanOrEqual(forms.y);
  expect(forms.y + forms.height).toBeLessThanOrEqual(sourceDetails.y);
  for (const box of [forms, name, sourceDetails]) {
    expect(box.x).toBeGreaterThanOrEqual(editor.x);
    expect(box.x + box.width).toBeLessThanOrEqual(editor.x + editor.width);
  }
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await birth.getByText('Details and source', { exact: true }).click();
  await expect(birth.getByLabel('Selected Conduit Form source', { exact: true })).toBeVisible();
});
