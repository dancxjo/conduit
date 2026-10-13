# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: static-body-application.spec.mjs >> static Handbook applications retain independent local Bodies through use, navigation, exclusion, reset and browser restart
- Location: proof/browser/static-body-application.spec.mjs:27:1

# Error details

```
Error: page.evaluate: Resulting promise was garbage collected.
```

# Test source

```ts
  1   | import { beginHandbookJourney } from "./handbook-journey.mjs";
  2   | import { expect, test } from "@playwright/test";
  3   | import { openStaticProfile, stageStaticApplications } from "./static-body-application-support.mjs";
  4   | 
  5   | test.describe.configure({ mode: "serial", retries: 0 });
  6   | 
  7   | async function current(page) {
> 8   |   return page.evaluate(() => globalThis.__conduitApplication.snapshot());
      |               ^ Error: page.evaluate: Resulting promise was garbage collected.
  9   | }
  10  | 
  11  | async function ready(page) {
  12  |   await expect(page.getByRole("button", { name: "Lull", exact: true })).toBeEnabled();
  13  |   await page.waitForFunction(() => Boolean(globalThis.__conduitApplication));
  14  |   const state = await current(page);
  15  |   for (const key of ["hostId", "bootId", "bodyId", "planId", "playId"]) expect(state[key], key).toBeTruthy();
  16  |   return state;
  17  | }
  18  | 
  19  | function renewed(first, next) {
  20  |   expect(next.hostId).toBe(first.hostId);
  21  |   expect(next.bodyId).toBe(first.bodyId);
  22  |   expect(next.bootId).not.toBe(first.bootId);
  23  |   expect(next.planId).not.toBe(first.planId);
  24  |   expect(next.playId).not.toBe(first.playId);
  25  | }
  26  | 
  27  | test("static Handbook applications retain independent local Bodies through use, navigation, exclusion, reset and browser restart", async ({}, testInfo) => {
  28  |   testInfo.setTimeout(180_000);
  29  |   const journey = await beginHandbookJourney();
  30  |   const site = await stageStaticApplications();
  31  |   const origin = new URL(site.url).origin;
  32  |   const application = new URL("handbook/", site.url).href;
  33  |   const otherApplication = new URL("second/", site.url).href;
  34  |   const profile = testInfo.outputPath("retained-browser-profile");
  35  |   let session;
  36  |   let retained;
  37  |   try {
  38  |     session = await openStaticProfile(profile, origin);
  39  |     const page = await session.context.newPage();
  40  |     await page.goto(application);
  41  |     const first = await ready(page);
  42  |     const opening = await page.evaluate(() => globalThis.__conduitApplication.opening);
  43  |     expect(opening.recovered).toBe(false);
  44  |     expect(opening.birthState).toBe("Lulled");
  45  |     await journey.capture(page, { id: "open", title: "Open your own Handbook",
  46  |       action: "Open the static Handbook in a fresh browser profile.",
  47  |       observation: "The Handbook creates a local Body, then wakes its resident application. The page offers lessons, editable source, and lifecycle controls." });
  48  |     await page.screenshot({ path: testInfo.outputPath("01-handbook-first-open.png"), fullPage: true });
  49  | 
  50  |     await page.getByRole("link", { name: /^Find your bearings/ }).click();
  51  |     await page.getByRole("heading", { name: "Start here", exact: true }).waitFor();
  52  |     expect((await ready(page)).bodyId).toBe(first.bodyId);
  53  |     await page.getByLabel("Choose an example", { exact: true }).selectOption({ label: "A clock you can stop" });
  54  |     await expect(page.getByRole("button", { name: "Try in my Handbook", exact: true })).toBeEnabled();
  55  |     const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
  56  |     const originalSource = await editor.inputValue();
  57  |     const highlighting = page.locator(".syntax-highlight code");
  58  |     await expect(highlighting).toHaveText(originalSource, { useInnerText: false });
  59  |     const initialTokens = await highlighting.locator("span").count();
  60  |     expect(initialTokens).toBeGreaterThan(0);
  61  |     const incomplete = "plot café {\n    # λ learning\n    clock: time/every(";
  62  |     await editor.fill(incomplete);
  63  |     await expect(highlighting).toHaveText(incomplete, { useInnerText: false });
  64  |     await expect(editor).toHaveAttribute("data-syntax-disposition", "accepted");
  65  |     expect(await highlighting.locator("span").count()).not.toBe(initialTokens);
  66  |     const beforeRefusal = await current(page);
  67  |     await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
  68  |     await expect(page.locator("[data-check]")).not.toHaveText("Edit the source, then check and try it in your body.");
  69  |     await expect(page.locator("[data-check]")).toHaveAttribute("data-refusal-code", /.+/);
  70  |     const afterRefusal = await current(page);
  71  |     expect(afterRefusal.bodyId).toBe(beforeRefusal.bodyId);
  72  |     expect(afterRefusal.playId).toBe(beforeRefusal.playId);
  73  |     expect(afterRefusal.installedPlots).toEqual(beforeRefusal.installedPlots);
  74  |     await journey.capture(page, { id: "check", title: "Check an unfinished edit",
  75  |       action: "Open Start here, choose the clock lesson, enter unfinished source, and press Try in my Handbook.",
  76  |       observation: "Live highlighting follows the Unicode edit. The checker reports a refusal; the existing Body, Play, and installed workset remain unchanged." });
  77  |     await editor.fill(originalSource);
  78  |     await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
  79  |     await expect.poll(async () => { const state = await current(page); return state.installedPlots.some(plot => plot.checked_plot_id === state.selectedPlot); }).toBe(true);
  80  |     let example = await ready(page);
  81  |     expect(example.bodyId).toBe(first.bodyId);
  82  |     expect(example.selectedPlot).toBeTruthy();
  83  |     expect(example.installedPlots.some(plot => plot.checked_plot_id === example.selectedPlot)).toBe(true);
  84  |     await journey.capture(page, { id: "try", title: "Run the clock lesson",
  85  |       action: "Restore the clock source and press Try in my Handbook.",
  86  |       observation: "The checked clock joins the same Body and runs through a newly admitted Plan and Play." });
  87  |     await page.getByRole("button", { name: "Open in Patchbay", exact: true }).first().click();
  88  |     await page.locator(".handbook-show").getByRole("button", { name: /clock-demo|A clock you can stop/i }).click();
  89  |     const graph = page.locator(".handbook-show [data-checked-plot-id]:visible");
  90  |     await expect(graph).toHaveAttribute("data-checked-plot-id", example.selectedPlot);
  91  |     await expect(graph.getByRole("group", { name: "Resident plot gears, ports, and cords" })).toBeVisible();
  92  |     await expect(graph).toHaveAttribute("data-body-plan-id", example.planId);
  93  |     await expect(graph).toHaveAttribute("data-active-play-id", example.playId);
  94  |     const outputPort = graph.getByRole("button", { name: /^output tick,/ });
  95  |     await outputPort.focus();
  96  |     await outputPort.press("Enter");
  97  |     await expect(outputPort).toHaveAttribute("aria-pressed", "true");
  98  |     await expect(outputPort).toBeFocused();
  99  |     let inspected = await current(page);
  100 |     expect(inspected.bodyId).toBe(first.bodyId);
  101 |     expect(inspected.selectedPlot).toEqual(example.selectedPlot);
  102 |     expect(inspected.foreground.checked_plot_id).not.toBe(example.selectedPlot);
  103 |     await journey.capture(page, { id: "inspect", title: "Inspect the running clock",
  104 |       action: "Press Open in Patchbay, choose the clock, focus its output port, and press Enter.",
  105 |       observation: "The resident Patchbay shows the clock's gears, ports, and cord. Its selected output port and graph identities belong to this Body's actual Plan and Play." });
  106 |     await page.screenshot({ path: testInfo.outputPath("02-real-patchbay.png"), fullPage: true });
  107 | 
  108 |     const originalExample = example;
```