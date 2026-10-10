import { beginHandbookJourney } from "./handbook-journey.mjs";
import { expect, test } from "@playwright/test";
import { openStaticProfile, stageStaticApplications } from "./static-body-application-support.mjs";

test.describe.configure({ mode: "serial", retries: 0 });

async function current(page) {
  return page.evaluate(() => globalThis.__conduitApplication.snapshot());
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
    await expect.poll(async () => { const state = await current(page); return state.installedPlots.some(plot => plot.checked_plot_id === state.selectedPlot); }).toBe(true);
    let example = await ready(page);
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
  testInfo.setTimeout(120_000);
  const site = await stageStaticApplications();
  const session = await openStaticProfile(testInfo.outputPath("lesson-profile"), new URL(site.url).origin);
  try {
    const page = await session.context.newPage();
    await page.goto(new URL("handbook/", site.url).href);
    const first = await ready(page);
    const selector = page.getByLabel("Choose an example", { exact: true });
    await expect(selector.locator("option")).toHaveCount(10);
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
    await expect(surface.locator('[data-resident-plot]:visible')).toContainText("The distances are not equal");
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
  testInfo.setTimeout(60_000);
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
    await expect(selector.locator("option")).toHaveCount(10);
    await expect(editor).toHaveValue(edited);
    await selector.selectOption("compare-distance-demo");
    await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
    await expect(page.locator('.handbook-show [data-resident-plot]:visible')).toContainText("The distances are equal");
    const upgraded = await current(page);
    expect(upgraded.bodyId).toBe(first.bodyId);
    expect(upgraded.installedPlots).toHaveLength(3);
    await selector.selectOption("clock-demo");
    await expect(editor).toHaveValue(edited);
    session.assertClean();
  } finally {
    await session.context.close();
    await site.close();
  }
});
