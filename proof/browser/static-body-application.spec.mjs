import { BROWSER_COLD_PREPARATION_MS } from "./browser-preparation.mjs";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { beginHandbookJourney } from "./handbook-journey.mjs";
import { expect, test } from "@playwright/test";
import { openStaticProfile, stageStaticApplications } from "./static-body-application-support.mjs";

test.describe.configure({ mode: "serial", retries: 0 });

async function current(page) {
  // Chromium's inspector can collect its unresolved evaluation promise even
  // while the actual snapshot remains live. Observe one real call's completion;
  // polling this record neither repeats the snapshot nor performs an action.
  await page.evaluate(() => {
    if (globalThis.__conduitProofSnapshot) throw new Error("Concurrent snapshot observation");
    const observation = { settled: false };
    globalThis.__conduitProofSnapshot = observation;
    globalThis.__conduitApplication.snapshot().then(value => {
      observation.value = value;
      observation.settled = true;
    }, error => {
      observation.error = { message: error.message, code: error.code };
      observation.settled = true;
    });
  });
  try {
    await page.waitForFunction(() => globalThis.__conduitProofSnapshot.settled, null, { timeout: 15_000 });
    return await page.evaluate(() => {
      const observation = globalThis.__conduitProofSnapshot;
      if (observation.error) throw Object.assign(new Error(observation.error.message), { code: observation.error.code });
      return observation.value;
    });
  } finally {
    await page.evaluate(() => { delete globalThis.__conduitProofSnapshot; });
  }
}

async function ready(page) {
  // The retained Candidate trace reached a real Playing Body after the default
  // five-second assertion budget expired during cold SDK/WASM acquisition.
  await expect(page.getByRole("button", { name: "Lull", exact: true })).toBeEnabled({ timeout: 30_000 });
  await page.waitForFunction(() => Boolean(globalThis.__conduitApplication));
  const state = await current(page);
  for (const key of ["hostId", "bootId", "bodyId", "planId", "playId"]) expect(state[key], key).toBeTruthy();
  return state;
}

function renewed(first, next) {
  expect(next.hostId).toBe(first.hostId);
  expect(next.bodyId).toBe(first.bodyId);
  expect(next.bootId).not.toBe(first.bootId);
  expect(next.planId).not.toBe(first.planId);
  expect(next.playId).not.toBe(first.playId);
}

test("static Handbook applications retain independent local Bodies through use, navigation, exclusion, reset and browser restart", async ({}, testInfo) => {
  testInfo.setTimeout(180_000);
  const journey = await beginHandbookJourney();
  const site = await stageStaticApplications();
  const origin = new URL(site.url).origin;
  const application = new URL("handbook/", site.url).href;
  const otherApplication = new URL("second/", site.url).href;
  const profile = testInfo.outputPath("retained-browser-profile");
  let session;
  let retained;
  try {
    session = await openStaticProfile(profile, origin);
    const page = await session.context.newPage();
    await page.goto(application);
    const first = await ready(page);
    const opening = await page.evaluate(() => globalThis.__conduitApplication.opening);
    expect(opening.recovered).toBe(false);
    expect(opening.birthState).toBe("Lulled");
    await journey.capture(page, { id: "open", title: "Open your own Handbook",
      action: "Open the static Handbook in a fresh browser profile.",
      observation: "The Handbook creates a local Body, then wakes its resident application. The page offers lessons, editable source, and lifecycle controls." });
    await page.screenshot({ path: testInfo.outputPath("01-handbook-first-open.png"), fullPage: true });

    await page.getByRole("link", { name: /^Find your bearings/ }).click();
    await page.getByRole("heading", { name: "Start here", exact: true }).waitFor();
    expect((await ready(page)).bodyId).toBe(first.bodyId);
    await page.getByLabel("Choose an example", { exact: true }).selectOption({ label: "A ticker you can stop" });
    await expect(page.getByRole("button", { name: "Try in my Handbook", exact: true })).toBeEnabled();
    const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
    const originalSource = await editor.inputValue();
    const highlighting = page.locator(".syntax-highlight code");
    await expect(highlighting).toHaveText(originalSource, { useInnerText: false });
    const initialTokens = await highlighting.locator("span").count();
    expect(initialTokens).toBeGreaterThan(0);
    const incomplete = "plot café {\n    # λ learning\n    clock: time/every(";
    await editor.fill(incomplete);
    await expect(highlighting).toHaveText(incomplete, { useInnerText: false });
    await expect(editor).toHaveAttribute("data-syntax-disposition", "accepted");
    expect(await highlighting.locator("span").count()).not.toBe(initialTokens);
    const beforeRefusal = await current(page);
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    await expect(page.locator("[data-check]")).not.toHaveText("Edit the source, then check and try it in your body.");
    await expect(page.locator("[data-check]")).toHaveAttribute("data-refusal-code", /.+/);
    const afterRefusal = await current(page);
    expect(afterRefusal.bodyId).toBe(beforeRefusal.bodyId);
    expect(afterRefusal.playId).toBe(beforeRefusal.playId);
    expect(afterRefusal.installedPlots).toEqual(beforeRefusal.installedPlots);
    await journey.capture(page, { id: "check", title: "Check an unfinished edit",
      action: "Open Start here, choose the clock lesson, enter unfinished source, and press Try in my Handbook.",
      observation: "Live highlighting follows the Unicode edit. The checker reports a refusal; the existing Body, Play, and installed workset remain unchanged." });
    await editor.fill(originalSource);
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    // Controls reenable after installation and the actual SDK current() settle.
    let example = await ready(page);
    await expect.poll(async () => { const state = await current(page); return state.installedPlots.some(plot => plot.checked_plot_id === state.selectedPlot); }).toBe(true);
    expect(example.bodyId).toBe(first.bodyId);
    expect(example.selectedPlot).toBeTruthy();
    expect(example.installedPlots.some(plot => plot.checked_plot_id === example.selectedPlot)).toBe(true);
    await journey.capture(page, { id: "try", title: "Run the clock lesson",
      action: "Restore the clock source and press Try in my Handbook.",
      observation: "The checked clock joins the same Body and runs through a newly admitted Plan and Play." });
    await page.getByRole("button", { name: "Open in Patchbay", exact: true }).first().click();
    await page.locator(".handbook-show").getByRole("button", { name: /clock-demo|A ticker you can stop/i }).click();
    const graph = page.locator(".handbook-show [data-checked-plot-id]:visible");
    await expect(graph).toHaveAttribute("data-checked-plot-id", example.selectedPlot);
    await expect(graph.getByRole("group", { name: "Resident plot gears, ports, and cords" })).toBeVisible();
    await expect(graph).toHaveAttribute("data-body-plan-id", example.planId);
    await expect(graph).toHaveAttribute("data-active-play-id", example.playId);
    const outputPort = graph.getByRole("button", { name: /^output tick,/ });
    await outputPort.focus();
    await outputPort.press("Enter");
    await expect(outputPort).toHaveAttribute("aria-pressed", "true");
    await expect(outputPort).toBeFocused();
    let inspected = await current(page);
    expect(inspected.bodyId).toBe(first.bodyId);
    expect(inspected.selectedPlot).toEqual(example.selectedPlot);
    expect(inspected.foreground.checked_plot_id).not.toBe(example.selectedPlot);
    await journey.capture(page, { id: "inspect", title: "Inspect the running clock",
      action: "Press Open in Patchbay, choose the clock, focus its output port, and press Enter.",
      observation: "The resident Patchbay shows the clock's gears, ports, and cord. Its selected output port and graph identities belong to this Body's actual Plan and Play." });
    await page.screenshot({ path: testInfo.outputPath("02-real-patchbay.png"), fullPage: true });

    const originalExample = example;
    const editedSource = originalSource.replace("time/every(1s)", "time/every(2s)");
    expect(editedSource).not.toBe(originalSource);
    await editor.fill(editedSource);
    await expect(page.locator("[data-check]")).toHaveText("Edited source — not checked or applied yet. Try it to check and apply the change.");
    await expect(graph).toHaveAttribute("data-checked-plot-id", originalExample.selectedPlot);
    await expect(highlighting).toHaveText(editedSource, { useInnerText: false });
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    // Applying the edit returns to the selected Plot. Its visible checker
    // result settles before the runtime snapshot; Patchbay is reopened below.
    await expect(page.locator("[data-check]")).toHaveText("Checked clock-demo.");
    example = await ready(page);
    expect(example.bodyId).toBe(originalExample.bodyId);
    expect(example.selectedPlot).not.toBe(originalExample.selectedPlot);
    expect(example.planId).not.toBe(originalExample.planId);
    expect(example.playId).not.toBe(originalExample.playId);
    expect(example.installedPlots).toHaveLength(originalExample.installedPlots.length);
    expect(example.installedPlots.some(plot => plot.checked_plot_id === originalExample.selectedPlot)).toBe(false);
    expect(example.installedPlots.some(plot => plot.checked_plot_id === example.selectedPlot)).toBe(true);
    await journey.capture(page, { id: "edit", title: "Change the clock's timing",
      action: "Change time/every(1s) to time/every(2s), then press Try in my Handbook.",
      observation: "The same Body replaces the clock with the newly checked source and admits a new Plan and Play. The workset does not grow with a duplicate clock." });
    await page.getByLabel("Choose an example", { exact: true }).selectOption({ label: "Turn keystrokes into text" });
    await expect(editor).not.toHaveValue(editedSource);
    await expect(page.getByLabel("Choose an example", { exact: true })).toBeEnabled();
    await page.getByLabel("Choose an example", { exact: true }).selectOption({ label: "A ticker you can stop" });
    await expect(editor).toHaveValue(editedSource);
    await page.getByRole("button", { name: "Open in Patchbay", exact: true }).click();
    await page.locator(".handbook-show").getByRole("button", { name: /clock-demo|A ticker you can stop/i }).click();
    await expect(graph).toHaveAttribute("data-checked-plot-id", example.selectedPlot);
    await expect(graph).toHaveAttribute("data-body-plan-id", example.planId);
    await expect(graph).toHaveAttribute("data-active-play-id", example.playId);
    inspected = await current(page);
    await page.screenshot({ path: testInfo.outputPath("02b-edited-clock-patchbay.png"), fullPage: true });

    await page.getByRole("button", { name: "Lull", exact: true }).click();
    await expect(page.getByRole("button", { name: "Wake", exact: true })).toBeEnabled();
    const lulled = await current(page);
    expect(lulled.bodyId).toBe(first.bodyId);
    expect(lulled.playId ?? null).toBeNull();
    await journey.capture(page, { id: "lull", title: "Bring the Body to rest",
      action: "After checking that the edit survives switching lessons and inspecting the new graph, press Lull.",
      observation: "Execution ends and Wake becomes available. The Body retains the edited clock without an active Play." });
    await page.getByRole("button", { name: "Wake", exact: true }).click();
    const awake = await ready(page);
    expect(awake.playId).not.toBe(inspected.playId);
    await page.reload();
    const reloaded = await ready(page);
    renewed(awake, reloaded);
    await expect(editor).toHaveValue(editedSource);
    expect(reloaded.selectedPlot).toEqual(example.selectedPlot);
    expect(reloaded.installedPlots).toEqual(example.installedPlots);
    expect(reloaded.foreground).toEqual(inspected.foreground);
    await journey.capture(page, { id: "recover", title: "Return to your edited clock",
      action: "Press Wake, then reload the page.",
      observation: "The same Host and Body return with a fresh Boot, Plan, and Play. The two-second source, installed workset, and foreground selection survive." });

    const competitor = await session.context.newPage();
    await competitor.goto(application);
    await competitor.getByText(/already open|open in another (tab|window)/i).waitFor();
    await expect(competitor.getByRole("button", { name: "Lull", exact: true })).toBeDisabled();
    expect((await current(page)).bodyId).toBe(first.bodyId);

    const second = await session.context.newPage();
    await second.goto(otherApplication);
    const independent = await ready(second);
    expect(independent.hostId).toBe(first.hostId);
    expect(independent.bodyId).not.toBe(first.bodyId);
    await second.reload();
    renewed(independent, await ready(second));

    await page.locator('.handbook-application').getByText("Your body and browser", { exact: true }).click();
    await Promise.all([
      page.waitForNavigation({ waitUntil: "domcontentloaded" }),
      page.getByRole("button", { name: "Start my Handbook over", exact: true }).click(),
    ]);
    const reset = await ready(page);
    expect(reset.hostId).toBe(first.hostId);
    expect(reset.bodyId).not.toBe(first.bodyId);
    expect((await current(second)).bodyId).toBe(independent.bodyId);
    await page.locator('.handbook-application').getByText("Your body and browser", { exact: true }).click();
    await page.getByRole("button", { name: "Release this tab", exact: true }).click();
    await expect(page.locator("[data-session-status]")).toContainText("released your Handbook");
    await competitor.getByRole("button", { name: "Try again", exact: true }).click();
    retained = await ready(competitor);
    expect(retained.hostId).toBe(reset.hostId);
    expect(retained.bodyId).toBe(reset.bodyId);
    expect(retained.bootId).not.toBe(reset.bootId);
    session.assertClean();
    await session.context.close();
    session = null;

    session = await openStaticProfile(profile, origin);
    const returned = await session.context.newPage();
    await returned.goto(application);
    renewed(retained, await ready(returned));
    await returned.screenshot({ path: testInfo.outputPath("03-browser-reopened.png"), fullPage: true });
    session.assertClean();
    await session.context.close();
    session = null;

    session = await openStaticProfile(testInfo.outputPath("independent-browser-profile"), origin);
    const otherBrowser = await session.context.newPage();
    await otherBrowser.goto(application);
    const distinct = await ready(otherBrowser);
    expect(distinct.hostId).not.toBe(retained.hostId);
    expect(distinct.bodyId).not.toBe(retained.bodyId);
    session.assertClean();
    await journey.finish({ opening, first, beforeRefusal, afterRefusal, originalExample,
      edited: example, inspected, lulled, reloaded, independent, reset, retained, distinct });
  } finally {
    await session?.context.close();
    await site.close();
  }
});

test("Handbook lessons run text, arithmetic, logic and exact quantities in the local Body", async ({}, testInfo) => {
  testInfo.setTimeout(120_000 + BROWSER_COLD_PREPARATION_MS);
  const site = await stageStaticApplications();
  const session = await openStaticProfile(testInfo.outputPath("lesson-profile"), new URL(site.url).origin);
  try {
    const page = await session.context.newPage();
    await page.goto(new URL("handbook/", site.url).href);
    const first = await ready(page);
    const selector = page.getByLabel("Choose an example", { exact: true });
    await expect(selector.locator("option")).toHaveCount(11);
    const surface = page.locator(".handbook-show");
    const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
    const cases = [
      ["hello-demo", "HELLO, WORLD."],
      ["scale-demo", "3.000000"],
      ["logic-demo", "false"],
      ["convert-pitch-demo", "Exactly 1000 Hz"],
      ["compare-distance-demo", "The distances are equal"],
      ["temperature-change-demo", "The temperature change is exactly 5 K"],
      ["inexact-conversion-demo", "Conversion refused: inexact"],
    ];
    for (const [name, result] of cases) {
      await selector.selectOption(name);
      await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
      await expect(page.locator("[data-check]")).toHaveText(`Checked ${name}.`);
      // Checked Source is reported before the replacement Body has prepared.
      await expect(page.getByRole("button", { name: "Try in my Handbook", exact: true }))
        .toBeEnabled({ timeout: BROWSER_COLD_PREPARATION_MS });
      await expect(surface.locator('[data-resident-plot]:visible')).toContainText(result);
      const state = await current(page);
      expect(state.bodyId).toBe(first.bodyId);
      expect(state.installedPlots).toHaveLength(3); // Tour, Patchbay and one lesson.
      expect(state.installedPlots.some(plot => plot.checked_plot_id === state.selectedPlot)).toBe(true);
    }
    await selector.selectOption("text-lab");
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    await expect(page.locator("[data-check]")).toHaveText("Checked text-lab.");
    await surface.focus();
    await page.keyboard.type("hello");
    await expect(surface.locator('[data-resident-plot]:visible')).toContainText("O");

    // An edited unit request changes the computed output and survives recovery.
    await selector.selectOption("compare-distance-demo");
    const original = await editor.inputValue();
    expect(original).toContain("right = 0.001km");
    const editedSource = original.replace("right = 0.001km", "right = 0.002km");
    expect(editedSource).not.toBe(original);
    await editor.fill(editedSource);
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    // Editing source prepares a replacement realization before ordinary input resumes.
    await expect(surface.locator('[data-resident-plot]:visible')).toContainText("The distances are not equal", { timeout: BROWSER_COLD_PREPARATION_MS });
    const edited = await current(page);
    await page.reload();
    const recovered = await ready(page);
    expect(recovered.bodyId).toBe(first.bodyId);
    expect(recovered.selectedPlot).toBe(edited.selectedPlot);
    await expect(selector).toHaveValue("compare-distance-demo");
    await expect(editor).toHaveValue(editedSource);
    await expect(surface.locator('[data-resident-plot]:visible')).toContainText("The distances are not equal");
    session.assertClean();
  } finally {
    await session.context.close();
    await site.close();
  }
});

test("an existing two-lesson Handbook gains new choices without resetting its Body or saved edits", async ({}, testInfo) => {
  testInfo.setTimeout(60_000 + 2 * BROWSER_COLD_PREPARATION_MS);
  const site = await stageStaticApplications({ legacyLessonInventory: true });
  const session = await openStaticProfile(testInfo.outputPath("upgraded-profile"), new URL(site.url).origin);
  try {
    const page = await session.context.newPage();
    await page.goto(new URL("legacy/", site.url).href);
    const first = await ready(page);
    const selector = page.getByLabel("Choose an example", { exact: true });
    const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
    await expect(selector.locator("option")).toHaveCount(2);
    const edited = (await editor.inputValue()).replace("time/every(1s)", "time/every(2s)");
    await editor.fill(edited);
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    await expect(page.locator("[data-check]")).toHaveText("Checked clock-demo.");
    await page.locator(".handbook-application").getByText("Your body and browser", { exact: true }).click();
    await page.getByRole("button", { name: "Release this tab", exact: true }).click();
    await expect(page.locator("[data-session-status]")).toContainText("released your Handbook");

    await page.goto(new URL("handbook/", site.url).href);
    expect((await ready(page)).bodyId).toBe(first.bodyId);
    await expect(selector.locator("option")).toHaveCount(11);
    await expect(editor).toHaveValue(edited);
    await selector.selectOption("compare-distance-demo");
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    // Installing a newly offered lesson prepares a replacement Body before output.
    await expect(page.getByRole("button", { name: "Try in my Handbook", exact: true }))
      .toBeEnabled({ timeout: BROWSER_COLD_PREPARATION_MS });
    await expect(page.locator('.handbook-show [data-resident-plot]:visible')).toContainText("The distances are equal");
    const upgraded = await current(page);
    expect(upgraded.bodyId).toBe(first.bodyId);
    expect(upgraded.installedPlots).toHaveLength(3);
    await selector.selectOption("thermostat/main");
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    await expect(page.getByRole("button", { name: "Try in my Handbook", exact: true }))
      .toBeEnabled({ timeout: BROWSER_COLD_PREPARATION_MS });
    await expect(page.locator('.handbook-show conduit-owner-face')
      .getByText("Sensor unavailable", { exact: true })).toBeVisible();
    expect((await current(page)).bodyId).toBe(first.bodyId);
    await selector.selectOption("clock-demo");
    await expect(editor).toHaveValue(edited);
    session.assertClean();
  } finally {
    await session.context.close();
    await site.close();
  }
});


test("malformed packaged Handbook Plot leaves an editable recovery route into a real Body", async ({}, testInfo) => {
  testInfo.setTimeout(120_000);
  const site = await stageStaticApplications({ malformedLessonSource: true });
  const session = await openStaticProfile(testInfo.outputPath("broken-package-profile"), new URL(site.url).origin);
  try {
    const page = await session.context.newPage();
    await page.goto(new URL("broken/", site.url).href);
    await expect(page.locator("[data-session-status]")).toContainText("CND-FRM-051");
    const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
    const chooser = page.getByLabel("Choose an example", { exact: true });
    await expect(editor).toBeEnabled();
    await expect(chooser).toBeEnabled();
    await expect(page.getByRole("button", { name: "Try in my Handbook", exact: true })).toBeEnabled();
    await expect(editor).toHaveValue(/source = "1kHz"/);
    await expect(chooser.locator("option")).toHaveCount(13); // Two resident Plots plus eleven lessons.
    const repaired = (await editor.inputValue()).replace('source = "1kHz"', "source = 1kHz");
    await editor.fill(repaired);
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    await expect(page.locator("[data-check]")).toContainText("Source checks. Correction saved");
    await expect(page.getByRole("button", { name: "Try again", exact: true })).toBeEnabled();
    await page.getByRole("button", { name: "Try again", exact: true }).click();
    await ready(page);
    await page.getByLabel("Choose an example", { exact: true }).selectOption("convert-pitch-demo");
    await expect(page.getByRole("textbox", { name: "Plot source", exact: true }))
      .toHaveValue(repaired);
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    await expect(page.locator("[data-check]")).toHaveText("Checked convert-pitch-demo.");
    await expect(page.locator(".handbook-show [data-resident-plot]:visible"))
      .toContainText("Exactly 1000 Hz");
    session.assertClean();
  } finally {
    await session.context.close();
    await site.close();
  }
});

test("Handbook checker spans identify edited source without replacing the active Play", async ({}, testInfo) => {
  // Cold Body preparation has its own finite allowance before checker interactions.
  testInfo.setTimeout(20_000 + BROWSER_COLD_PREPARATION_MS);
  const site = await stageStaticApplications();
  const session = await openStaticProfile(testInfo.outputPath("checker-span-profile"), new URL(site.url).origin);
  try {
    const page = await session.context.newPage();
    await page.goto(new URL("handbook/", site.url).href);
    const before = await ready(page);
    const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
    await editor.fill("clock {");
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    const diagnostic = page.locator("[data-check]");
    await expect(diagnostic).toContainText("edited source clock-demo — checking refused");
    await expect(diagnostic).toContainText("line 1, column 1");
    await expect(editor).toBeEnabled();
    await expect(editor).toHaveValue("clock {");
    const after = await current(page);
    for (const key of ["bodyId", "planId", "playId"]) expect(after[key]).toBe(before[key]);
    session.assertClean();
  } finally {
    await session.context.close();
    await site.close();
  }
});

for (const position of ["first", "middle"]) {
  test(`malformed ${position} Handbook lesson preserves independent source inspection and checks`, async ({}, testInfo) => {
    testInfo.setTimeout(120_000);
    const site = await stageStaticApplications({ malformedLessonSource: position });
    const session = await openStaticProfile(testInfo.outputPath(`malformed-${position}-profile`), new URL(site.url).origin);
    try {
      const page = await session.context.newPage();
      await page.goto(new URL("broken/", site.url).href);
      await expect(page.locator("[data-session-status]")).toContainText("CND-FRM-055");
      await expect(page.locator("[data-session-status]")).toContainText("Packaged source checking refused");
      const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
      const chooser = page.getByLabel("Choose an example", { exact: true });
      await expect(chooser).toBeEnabled();
      await expect(chooser.locator("option")).toHaveCount(site.sourceInspection.length);
      for (const candidate of site.sourceInspection) {
        await chooser.selectOption({ label: `${candidate.title} (packaged)` });
        await expect(editor).toBeEnabled();
        await expect(editor).toHaveValue(candidate.source);
      }
      const malformed = site.sourceInspection.find(plot => plot.malformed);
      await chooser.selectOption({ label: `${malformed.title} (packaged)` });
      await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
      await expect(page.locator("[data-check]")).toContainText("CND-FRM-055");
      await expect(page.locator("[data-check]")).toContainText(`packaged source ${malformed.name}`);
      const sibling = site.sourceInspection.find(plot => plot.lesson && !plot.malformed);
      await chooser.selectOption({ label: `${sibling.title} (packaged)` });
      await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
      await expect(page.locator("[data-check]")).toContainText("Source checks.");
      await chooser.selectOption({ label: `${malformed.title} (packaged)` });
      await expect(editor).toHaveValue(malformed.source);
      expect(await page.evaluate(async () => ({
        record: await globalThis.__conduitBrowserApplication.storage.readJson("body-continuity"),
        runnable: Boolean(globalThis.__conduitApplication),
      }))).toEqual({ record: null, runnable: false });
      session.assertClean();
    } finally {
      await session.context.close();
      await site.close();
    }
  });
}

test("refused Handbook recovery preserves original state until explicit replacement consent", async ({}, testInfo) => {
  testInfo.setTimeout(120_000);
  const site = await stageStaticApplications();
  const session = await openStaticProfile(testInfo.outputPath("broken-retained-profile"), new URL(site.url).origin);
  try {
    const page = await session.context.newPage();
    await page.goto(new URL("handbook/", site.url).href);
    const original = await ready(page);
    await page.getByRole("button", { name: "Lull", exact: true }).click();
    const retained = await page.evaluate(async () => {
      const store = globalThis.__conduitBrowserApplication.storage;
      const record = await store.readJson("body-continuity");
      const inventory = JSON.parse(record.source);
      const lesson = inventory.plots.find(plot => plot.entry === "convert-pitch-demo");
      if (!lesson) throw new Error("missing retained pitch lesson");
      const packagedSource = lesson.source;
      lesson.source = lesson.source.replace("source = 1kHz", 'source = "1kHz"');
      if (!lesson.source.includes('source = "1kHz"')) throw new Error("failed to seed syntax refusal");
      const changed = { ...record, source: JSON.stringify(inventory) };
      await store.writeJson("body-continuity", changed);
      return { record: changed, packagedSource, refusedSource: lesson.source };
    });
    await page.locator(".handbook-application").getByText("Your body and browser", { exact: true }).click();
    await page.getByRole("button", { name: "Release this tab", exact: true }).click();
    await page.reload();
    await expect(page.locator("[data-session-status]")).toContainText("CND-FRM-051");
    await expect(page.locator("[data-session-status]")).toContainText("Retained Body recovery refused");
    const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
    await expect(editor).toBeEnabled();
    await expect(editor).toHaveValue(retained.refusedSource);
    const chooser = page.getByLabel("Choose an example", { exact: true });
    await expect(chooser).toBeEnabled();
    const retainedChoice = await chooser.inputValue();
    const retainedLabel = await chooser.locator("option:checked").textContent();
    expect(retainedLabel).toContain("(retained)");
    await chooser.selectOption({ label: retainedLabel.replace("(retained)", "(packaged)") });
    await expect(editor).toHaveValue(retained.packagedSource);
    await chooser.selectOption(retainedChoice);
    await expect(editor).toHaveValue(retained.refusedSource);
    const continuity = () => page.evaluate(async () =>
      globalThis.__conduitBrowserApplication.storage.readJson("body-continuity"));
    expect(await continuity()).toEqual(retained.record);
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    await expect(page.locator("[data-check]")).toContainText("CND-FRM-051");
    await expect(page.locator("[data-check]")).toContainText("retained source convert-pitch-demo");
    expect(await continuity()).toEqual(retained.record);
    await expect(editor).toBeEnabled();
    const repaired = (await editor.inputValue()).replace('source = "1kHz"', "source = 1kHz");
    await editor.fill(repaired);
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    await expect(page.locator("[data-check]")).toContainText("Source checks. The retained Body has not been changed");
    await expect(page.getByRole("button", { name: "Download original retained Body record" })).toBeEnabled();
    expect(await continuity()).toEqual(retained.record);
    expect(await page.evaluate(() => Boolean(globalThis.__conduitApplication))).toBe(false);
    expect(original.bodyId).toBeTruthy();
    const downloadEvent = page.waitForEvent("download");
    await page.getByRole("button", { name: "Download original retained Body record", exact: true }).click();
    const download = await downloadEvent;
    expect(download.suggestedFilename()).toBe("conduit-handbook-retained-body.json");
    expect(await readFile(await download.path(), "utf8")).toBe(JSON.stringify(retained.record, null, 2));
    expect(await continuity()).toEqual(retained.record);
    const consent = page.getByRole("checkbox", { name: "I have saved what I need", exact: false });
    const replacement = page.getByRole("button", { name: "Start a new Handbook Body", exact: true });
    await expect(consent).not.toBeChecked();
    await expect(replacement).toBeDisabled();
    await consent.check();
    await expect(replacement).toBeEnabled();
    await consent.uncheck();
    await expect(replacement).toBeDisabled();
    expect(await continuity()).toEqual(retained.record);
    await consent.check();
    await replacement.click();
    const fresh = await ready(page);
    expect(fresh.bodyId).not.toBe(original.bodyId);
    expect(fresh.hostId).toBe(original.hostId);
    const freshRecord = await continuity();
    expect(freshRecord.source).not.toContain('source = \\"1kHz\\"');
    session.assertClean();
  } finally {
    await session.context.close();
    await site.close();
  }
});

for (const malformed of [false, true]) {
  test(`Handbook source editing precedes Host startup and preserves pending drafts (${malformed ? "refused" : "accepted"} bundle)`, async ({}, testInfo) => {
    testInfo.setTimeout(120_000);
    const site = await stageStaticApplications({ malformedLessonSource: malformed });
    const session = await openStaticProfile(testInfo.outputPath("opening-editor-profile"), new URL(site.url).origin);
    let release;
    const gate = new Promise(resolve => { release = resolve; });
    try {
      const page = await session.context.newPage();
      let downloads = 0;
      await page.route("**/sdk/bundle/runtime.wasm", async route => {
        // The first fetch admits the real application package. Pause only the
        // second fetch, which belongs to the genuine SDK's Host startup.
        if (++downloads === 2) await gate;
        await route.continue();
      });
      await page.goto(new URL(malformed ? "broken/" : "handbook/", site.url).href, { waitUntil: "commit" });
      await expect.poll(() => downloads, { timeout: 30_000 }).toBe(2);
      const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
      const chooser = page.getByLabel("Choose an example", { exact: true });
      await expect(editor).toBeEnabled();
      await expect(chooser).toBeEnabled();
      await expect(page.getByRole("button", { name: "Try in my Handbook", exact: true })).toBeDisabled();
      await chooser.selectOption("clock-demo");
      const draft = `${await editor.inputValue()}\n# pending-startup-draft\n`;
      await editor.fill(draft);
      await expect(page.locator("[data-check]")).toContainText("not checked or applied");
      release();
      if (malformed) {
        await expect(page.locator("[data-session-status]")).toContainText("CND-FRM-051");
        await expect(page.getByRole("button", { name: "Try in my Handbook", exact: true })).toBeEnabled();
      } else {
        await ready(page);
        const record = await page.evaluate(() => globalThis.__conduitBrowserApplication.storage.readJson("body-continuity"));
        expect(record.source).not.toContain("pending-startup-draft");
      }
      await expect(editor).toBeEnabled();
      await expect(chooser).toBeEnabled();
      await expect(editor).toHaveValue(draft);
      session.assertClean();
    } finally {
      release();
      await session.context.close();
      await site.close();
    }
  });
}

// Source-inspection fixture: no packaged WASM or Body admission is claimed.
for (const checker of [false, true]) {
test(`Handbook recovery distinguishes saved corrections from original packaged source (checker: ${checker})`, async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const original = 'plot pitch\n# original packaged "1kHz"\n';
  const corrected = 'plot pitch\n# saved correction 1kHz\n';
  const originalDigest = 'sha256:' + createHash('sha256').update(original).digest('hex');
  await page.evaluate(async ({ original, corrected, checker, originalDigest }) => {
    const { startApplication } = await import('/targets/browser/handbook/application.mjs');
    document.body.innerHTML = '<main><section data-handbook-application></section></main>';
    globalThis.storageWrites = [];
    await startApplication({
      manifest: checker ? { applicationId: 'fixture/handbook', packageDigest: `sha256:${'b'.repeat(64)}`,
        resources: [{ role: 'pitch-source', sha256: originalDigest }] } : undefined,
      text: role => role === 'birth-specification' ? JSON.stringify({ name: 'Fixture', initialPlots: ['pitch'],
        plots: [{ name: 'pitch', title: 'Pitch', role: 'pitch-source', lesson: 'Inspect pitch' }] }) : original,
      storage: {
        readJson: async key => key === 'handbook-source-rescue'
          ? [{ name: 'pitch', original, source: corrected }] : null,
        writeJson: async (...args) => { globalThis.storageWrites.push(args); },
      },
      browser: async () => {
        if (!checker) throw new Error('Fixture Host unavailable');
        return { plot: value => ({ check: async () => value.startsWith('{')
          ? { ok: false, refusal: { code: 'FixtureInventoryRefused', message: 'Fixture inventory refused' }, diagnostics: [] }
          : { ok: true, plots: [{ name: "pitch" }] } }) };
      },
    });
  }, { original, corrected, checker, originalDigest });
  const chooser = page.getByRole('combobox', { name: 'Choose an example' });
  const editor = page.getByRole('textbox', { name: 'Plot source', exact: true });
  await chooser.selectOption({ label: 'Pitch (saved correction)' });
  await expect(editor).toHaveValue(corrected);
  await editor.fill(`${corrected}# unsent correction draft é😀\n`);
  const exportSource = page.getByRole('button', { name: 'Download displayed source', exact: true });
  await expect(exportSource).toBeEnabled();
  const pendingDownload = page.waitForEvent('download');
  await exportSource.click();
  const download = await pendingDownload;
  expect(download.suggestedFilename()).toBe('pitch.conduit');
  expect(await readFile(await download.path(), 'utf8')).toBe(`${corrected}# unsent correction draft é😀\n`);
  await chooser.selectOption({ label: 'Pitch (packaged)' });
  await expect(editor).toHaveValue(original);
  const evidence = page.locator('[data-source-provenance]');
  await expect.poll(async () => JSON.parse(await evidence.textContent())).toMatchObject({
    application_id: checker ? 'fixture/handbook' : null,
    package_digest: checker ? `sha256:${'b'.repeat(64)}` : null,
    source_origin: 'packaged', packaged_source_sha256: checker ? originalDigest : null,
    displayed_source_sha256: originalDigest, local_edit: false,
  });
  await expect(editor).toBeEnabled();
  await chooser.selectOption({ label: 'Pitch (saved correction)' });
  await expect(editor).toHaveValue(`${corrected}# unsent correction draft é😀\n`);
  await expect.poll(async () => JSON.parse(await evidence.textContent())).toMatchObject({
    source_origin: 'saved correction', local_edit: true,
    displayed_source_sha256: 'sha256:' + createHash('sha256').update(`${corrected}# unsent correction draft é😀\n`).digest('hex'),
  });
  expect(await page.evaluate(() => globalThis.storageWrites)).toEqual([]);
  if (checker) {
    await page.getByRole('button', { name: 'Try in my Handbook', exact: true }).click();
    await expect(page.locator('[data-check]')).toContainText('Correction saved');
    await expect.poll(async () => JSON.parse(await evidence.textContent()).local_edit).toBe(false);
    expect(await page.evaluate(() => globalThis.storageWrites)).toEqual([['handbook-source-rescue', [
      { name: 'pitch', original, source: `${corrected}# unsent correction draft é😀\n` },
    ]]]);
  }
});
}

// A fixture checker isolates recovery persistence from Body/runtime admission.
for (const firstFails of [false, true]) {
test(`Handbook correction saves retain sibling edits while a write is pending (first fails: ${firstFails})`, async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async firstFails => {
    const { startApplication } = await import('/targets/browser/handbook/application.mjs');
    document.body.innerHTML = '<main><section data-handbook-application></section></main>';
    globalThis.savedCorrections = [];
    globalThis.checkedSources = [];
    globalThis.saveCalls = 0;
    await startApplication({
      text: role => role === 'birth-specification' ? JSON.stringify({ name: 'Fixture', initialPlots: ['first'],
        plots: ['first', 'second'].map(name => ({ name, title: name, role: name, lesson: name })) }) : `original ${role}`,
      storage: {
        readJson: async () => null,
        writeJson: async (key, value) => {
          if (++globalThis.saveCalls === 1) {
            await new Promise(resolve => { globalThis.releaseSave = resolve; });
            if (firstFails) throw new Error('Fixture storage failure');
          }
          globalThis.savedCorrections.push({ key, value: structuredClone(value) });
        },
      },
      browser: async () => ({ plot: value => ({ check: async () => {
        globalThis.checkedSources.push(value);
        return value.startsWith('{')
          ? { ok: false, refusal: { code: 'FixtureInventoryRefused', message: 'Fixture inventory refused' }, diagnostics: [] }
          : { ok: true, plots: [{ name: value.split(" ").at(-1) }] };
      } }) }),
    });
  }, firstFails);
  const chooser = page.getByRole('combobox', { name: 'Choose an example' });
  const editor = page.getByRole('textbox', { name: 'Plot source', exact: true });
  const save = page.getByRole('button', { name: 'Try in my Handbook', exact: true });
  await chooser.selectOption({ label: 'first (packaged)' });
  await editor.fill('corrected first');
  await save.click();
  await expect.poll(() => page.evaluate(() => typeof globalThis.releaseSave)).toBe('function');
  await chooser.selectOption({ label: 'second (packaged)' });
  await editor.fill('corrected second');
  await save.click();
  await expect.poll(() => page.evaluate(() => globalThis.checkedSources.includes('corrected second'))).toBe(true);
  expect(await page.evaluate(() => globalThis.saveCalls)).toBe(1);
  await page.evaluate(() => globalThis.releaseSave());
  await expect.poll(() => page.evaluate(() => globalThis.savedCorrections.length)).toBe(firstFails ? 1 : 2);
  expect(await page.evaluate(() => globalThis.savedCorrections.at(-1))).toEqual({
    key: 'handbook-source-rescue', value: [
      ...firstFails ? [] : [{ name: 'first', original: 'original first', source: 'corrected first' }],
      { name: 'second', original: 'original second', source: 'corrected second' },
    ],
  });
  await expect(page.locator('[data-check]')).toContainText('Correction saved');
});
}

for (const storageFails of [false, true]) {
  test(`Handbook source inspection precedes rescue storage (failure: ${storageFails})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(storageFails => {
      globalThis.pendingHandbook = import('/targets/browser/handbook/application.mjs').then(({ startApplication }) => {
        document.body.innerHTML = '<main><section data-handbook-application></section></main>';
        return startApplication({
          text: role => role === 'birth-specification' ? JSON.stringify({ name: 'Fixture', initialPlots: ['first'],
            plots: ['first', 'second'].map(name => ({ name, title: name, role: name, lesson: name })) }) : `original ${role}`,
          storage: {
            readJson: async key => {
              if (key !== 'handbook-source-rescue') return null;
              await new Promise(resolve => { globalThis.releaseRescueRead = resolve; });
              if (storageFails) throw new Error('Fixture storage read failure');
              return ['first', 'second'].map(name => ({ name, original: `original ${name}`, source: `saved ${name}` }));
            },
            writeJson: async () => { throw new Error('Inspection must not write storage'); },
          },
          browser: async () => { throw new Error('Fixture Host unavailable'); },
        });
      });
    }, storageFails);
    const chooser = page.getByRole('combobox', { name: 'Choose an example' });
    const editor = page.getByRole('textbox', { name: 'Plot source', exact: true });
    await expect(editor).toBeEnabled();
    await expect(chooser).toBeEnabled();
    await expect(editor).toHaveValue('original first');
    await editor.fill('unsent first draft');
    await chooser.selectOption('second');
    await editor.fill('unsent second draft');
    await page.evaluate(async () => { globalThis.releaseRescueRead(); await globalThis.pendingHandbook; });
    await expect(editor).toHaveValue('unsent second draft');
    await chooser.selectOption({ label: `first (${storageFails ? 'packaged' : 'saved correction'})` });
    await expect(editor).toHaveValue('unsent first draft');
    if (!storageFails) {
      await chooser.selectOption({ label: 'first (packaged)' });
      await expect(editor).toHaveValue('original first');
    }
    await expect(editor).toBeEnabled();
  });
}

// Production editor orchestration with a fixture SDK; no runtime proof claim.
test('Handbook retains unsent lesson drafts across startup preferences and selection', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(() => {
    globalThis.pendingHandbook = import('/targets/browser/handbook/application.mjs').then(({ startApplication }) => {
      document.body.innerHTML = '<main><section data-handbook-application></section></main>';
      globalThis.bodyTransitions = [];
      const plots = ['first', 'second'].map(name => ({ name, checkedPlotId: `fixture/${name}`, source: `original ${name}` }));
      const checked = { ok: true, plots };
      const body = {
        id: 'fixture/body', plots: async () => checked,
        current: async () => ({ evidence: { body: { state: 'Awake',
          workset: { plots: [{ checked_plot_id: 'fixture/first' }] } } } }),
        select: async () => { globalThis.bodyTransitions.push('select'); },
        wake: async () => {
          globalThis.bodyTransitions.push('wake');
          return { state: 'playing', dispatch: () => new Promise(() => {}) };
        },
      };
      return startApplication({
        text: role => role === 'birth-specification' ? JSON.stringify({ name: 'Fixture', initialPlots: ['first'],
          plots: ['first', 'second'].map(name => ({ name, title: name, role: name, lesson: name })) }) : `original ${role}`,
        storage: {
          readJson: async key => {
            if (key !== 'selected-example') return null;
            await new Promise(resolve => { globalThis.releasePreferenceRead = resolve; });
            return 'first';
          },
          writeJson: async () => {}, durability: async () => ({ fixture: true }),
        },
        browser: async () => ({ recover: async () => body, attachEditor: () => ({ render() {}, destroy() {} }),
          plot: value => ({ check: async () => value.startsWith('{') ? checked
            : { ok: false, refusal: { code: 'FixtureSyntaxRefused', message: 'Fixture draft refused' }, diagnostics: [] } }) }),
      });
    });
  });
  const chooser = page.getByRole('combobox', { name: 'Choose an example' });
  const editor = page.getByRole('textbox', { name: 'Plot source', exact: true });
  await expect.poll(() => page.evaluate(() => typeof globalThis.releasePreferenceRead)).toBe('function');
  await editor.fill('unsent first draft');
  await chooser.selectOption('second');
  await editor.fill('unsent second draft');
  await page.evaluate(async () => { globalThis.releasePreferenceRead(); await globalThis.pendingHandbook; });
  await expect(chooser).toHaveValue('second');
  await expect(editor).toHaveValue('unsent second draft');
  await chooser.selectOption('first');
  await expect(editor).toHaveValue('unsent first draft');
  await chooser.selectOption('second');
  await expect(editor).toHaveValue('unsent second draft');
  await page.getByRole('button', { name: 'Try in my Handbook', exact: true }).click();
  await expect(page.locator('[data-check]')).toHaveAttribute('data-refusal-code', 'FixtureSyntaxRefused');
  await expect(editor).toHaveValue('unsent second draft');
  expect(await page.evaluate(() => globalThis.bodyTransitions)).toEqual(['select', 'wake']);
});

// Recovery must preserve the selected lesson identity as well as valid syntax.
test('Handbook recovery refuses a correction that renames the selected Plot', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async () => {
    const { startApplication } = await import('/targets/browser/handbook/application.mjs');
    document.body.innerHTML = '<main><section data-handbook-application></section></main>';
    globalThis.storageWrites = [];
    await startApplication({
      text: role => role === 'birth-specification' ? JSON.stringify({ name: 'Fixture', initialPlots: ['pitch'],
        plots: [{ name: 'pitch', title: 'Pitch', role: 'pitch', lesson: 'Pitch' }] }) : 'plot pitch',
      storage: { readJson: async () => null, writeJson: async (...args) => globalThis.storageWrites.push(args) },
      browser: async () => ({ plot: value => ({ check: async () => value.startsWith('{')
        ? { ok: false, refusal: { code: 'FixtureInventoryRefused', message: 'Fixture inventory refused' }, diagnostics: [] }
        : { ok: true, plots: [{ name: value === 'plot renamed' ? 'renamed' : 'pitch' }] } }) }),
    });
  });
  const editor = page.getByRole('textbox', { name: 'Plot source', exact: true });
  const save = page.getByRole('button', { name: 'Try in my Handbook', exact: true });
  await editor.fill('plot renamed');
  await save.click();
  await expect(page.locator('[data-check]')).toHaveAttribute('data-refusal-code', 'LessonPlotNameChanged');
  expect(await page.evaluate(() => globalThis.storageWrites)).toEqual([]);
  await expect(editor).toHaveValue('plot renamed');
  await editor.fill('plot pitch');
  await save.click();
  await expect(page.locator('[data-check]')).toContainText('Correction saved');
  expect(await page.evaluate(() => globalThis.storageWrites)).toEqual([['handbook-source-rescue', [
    { name: 'pitch', original: 'plot pitch', source: 'plot pitch' },
  ]]]);
});

for (const oversized of [false, true, "header"]) {
  test(`static application loader bounds streamed manifests (oversized: ${oversized})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async oversized => {
      document.body.innerHTML = '<main id="workspace"></main>';
      const meta = document.createElement('meta');
      meta.name = 'conduit-application-package'; meta.content = '/fixture-application.json';
      document.head.append(meta);
      const maximum = 48 * 1024;
      const bytes = new TextEncoder().encode('{}' + ' '.repeat(maximum - 2));
      globalThis.streamEvidence = { pulls: 0, cancelled: false };
      const stream = new ReadableStream({
        pull(controller) {
          const count = ++globalThis.streamEvidence.pulls;
          if (count <= 2) controller.enqueue(bytes.slice((count - 1) * maximum / 2, count * maximum / 2));
          else if (oversized && count <= 4) controller.enqueue(new Uint8Array([32]));
          else controller.close();
        },
        cancel() { globalThis.streamEvidence.cancelled = true; },
      }, { highWaterMark: 0 });
      globalThis.fixtureStream = stream;
      globalThis.fetch = async () => new Response(stream, oversized === "header"
        ? { headers: { "content-length": String(maximum + 1) } } : {});
      await import('/targets/browser/host/assets/browser-application-loader.mjs');
    }, oversized);
    await expect(page.locator('#workspace')).toHaveText(oversized
      ? 'application package manifest exceeds its admitted bound' : 'browser application package schema is unsupported');
    expect(await page.evaluate(() => ({ ...globalThis.streamEvidence, locked: globalThis.fixtureStream.locked })))
      .toEqual({ pulls: oversized === "header" ? 0 : 3, cancelled: Boolean(oversized), locked: false });
  });
}

for (const redirected of [true, false]) {
  test(`static application loader keeps manifest fetch on its declared URL (redirect: ${redirected})`, async ({ page }) => {
    let destinationRequests = 0;
    page.on('request', request => {
      if (request.url().endsWith('/proof/browser/fixture-redirect-target.json')) destinationRequests++;
    });
    await page.route('**/proof/browser/fixture-application.json', route => route.fulfill(redirected
      ? { status: 302, headers: { location: '/proof/browser/fixture-redirect-target.json' } }
      : { contentType: 'application/json', body: '{}' }));
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async () => {
      document.body.innerHTML = '<main id="workspace"></main>';
      const meta = document.createElement('meta');
      meta.name = 'conduit-application-package'; meta.content = '/proof/browser/fixture-application.json';
      document.head.append(meta);
      await import('/targets/browser/host/assets/browser-application-loader.mjs');
    });
    await expect(page.locator('#workspace.error')).toBeVisible();
    expect(destinationRequests).toBe(0);
    await expect(page.locator('#workspace.error')).toHaveText(redirected
      ? 'Failed to fetch' : 'browser application package schema is unsupported');
  });
}

for (const kind of ['entry-kind', 'runtime-kind', 'module', 'classic-script', 'shared']) {
  test(`static application loader preflights dependency graph (${kind})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async kind => {
      document.body.innerHTML = '<main id="workspace"></main>';
      const meta = document.createElement('meta');
      meta.name = 'conduit-application-package'; meta.content = '/fixture-application.json';
      document.head.append(meta);
      const resource = (role, dependencies = []) => ({ role, kind: (kind === 'entry-kind' && role === 'application-module') || (kind === 'runtime-kind' && role === 'runtime') ? 'content' : role === 'runtime' ? 'wasm'
        : kind === 'classic-script' && role !== 'application-module' ? 'classic-script' : 'module',
        path: role + '.mjs', maximum_bytes: 1024, sha256: 'sha256:' + 'a'.repeat(64),
        dependencies: dependencies.map(role => ({ role, specifier: './' + role + '.mjs' })) });
      const manifest = { schema: 'conduit.browser/application-package@1', application_id: 'fixture/graph',
        package_digest: 'sha256:' + 'b'.repeat(64), state_compatibility: { identity: 'fixture/graph-state', version: 1 },
        host_implementations: ['browser/dom@1'], resources: [resource('application-module', kind === 'shared' ? ['second'] : []), resource('runtime'),
          resource('first', ['second']), resource('second', kind === 'shared' || kind.endsWith('-kind') ? [] : ['first'])] };
      if (kind !== 'shared') {
        const lines = ['conduit.browser/application-package-content@1', 'application\0' + manifest.application_id,
          'state\0' + manifest.state_compatibility.identity + '\0' + manifest.state_compatibility.version,
          'host-implementation\0browser/dom@1'];
        for (const resource of manifest.resources) lines.push('resource\0' + [resource.role, resource.kind, resource.path,
          resource.maximum_bytes, resource.sha256, resource.dependencies.map(dep => dep.role + '=' + dep.specifier).join(',')].join('\0'));
        const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(lines.join('\n') + '\n'));
        manifest.package_digest = 'sha256:' + [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('');
      }
      globalThis.resourceRequests = 0;
      globalThis.fetch = async url => {
        if (!String(url).endsWith('/fixture-application.json')) globalThis.resourceRequests++;
        return new Response(JSON.stringify(manifest));
      };
      await import('/targets/browser/host/assets/browser-application-loader.mjs');
    }, kind);
    await expect(page.locator('#workspace.error')).toHaveText(kind === 'shared'
      ? 'application package identity changed' : kind.endsWith('-kind')
        ? 'application package entry or runtime kind is invalid' : 'application resource dependency cycle refused');
    expect(await page.evaluate(() => globalThis.resourceRequests)).toBe(0);
  });
}

for (const refused of [true, false]) {
  test(`static application loader releases prepared module URLs (refused: ${refused})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async refused => {
      document.body.innerHTML = '<main id="workspace"></main>';
      const meta = document.createElement('meta');
      meta.name = 'conduit-application-package'; meta.content = '/fixture-application.json';
      document.head.append(meta);
      const encoder = new TextEncoder();
      const hash = async bytes => 'sha256:' + [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
        .map(byte => byte.toString(16).padStart(2, '0')).join('');
      const source = (refused ? '' : 'import "./helper.mjs";\n')
        + 'export async function startApplication() { document.querySelector("#workspace").textContent = "Started fixture"; }';
      const definitions = [
        ['application-module', 'module', 'entry.mjs', source, [{ role: 'helper', specifier: './helper.mjs' }]],
        ['helper', 'module', 'helper.mjs', 'export const ready = true;', []],
        ['runtime', 'wasm', 'runtime.wasm', 'fixture envelope', []],
      ];
      const resources = [];
      const bodies = new Map();
      for (const [role, kind, path, text, dependencies] of definitions) {
        const bytes = encoder.encode(text); bodies.set(path, bytes);
        resources.push({ role, kind, path, maximum_bytes: bytes.length, sha256: await hash(bytes), dependencies });
      }
      const manifest = { schema: 'conduit.browser/application-package@1', application_id: 'fixture/module-cleanup',
        state_compatibility: { identity: 'fixture/module-cleanup-state', version: 1 },
        host_implementations: ['browser/indexeddb@1'], resources };
      const lines = ['conduit.browser/application-package-content@1', 'application\0' + manifest.application_id,
        'state\0' + manifest.state_compatibility.identity + '\0' + 1, 'host-implementation\0browser/indexeddb@1'];
      for (const resource of resources) lines.push('resource\0' + [resource.role, resource.kind, resource.path,
        resource.maximum_bytes, resource.sha256, resource.dependencies.map(dep => dep.role + '=' + dep.specifier).join(',')].join('\0'));
      manifest.package_digest = await hash(encoder.encode(lines.join('\n') + '\n'));
      globalThis.fetch = async url => new Response(String(url).endsWith('/fixture-application.json')
        ? JSON.stringify(manifest) : bodies.get(new URL(url).pathname.split('/').at(-1)));
      globalThis.moduleUrls = { created: [], revoked: [] };
      const create = URL.createObjectURL.bind(URL), revoke = URL.revokeObjectURL.bind(URL);
      URL.createObjectURL = blob => { const url = create(blob); globalThis.moduleUrls.created.push(url); return url; };
      URL.revokeObjectURL = url => { globalThis.moduleUrls.revoked.push(url); revoke(url); };
      await import('/targets/browser/host/assets/browser-application-loader.mjs');
    }, refused);
    await expect(page.locator('#workspace')).toHaveText(refused
      ? 'application module application-module did not use its declared dependency' : 'Started fixture');
    const urls = await page.evaluate(() => globalThis.moduleUrls);
    expect(urls.created).toHaveLength(refused ? 1 : 2);
    expect(urls.revoked).toEqual(urls.created);
  });
}
