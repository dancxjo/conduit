# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: static-body-application.spec.mjs >> static Handbook applications retain independent local Bodies through use, navigation, exclusion, reset and browser restart
- Location: proof/browser/static-body-application.spec.mjs:53:1

# Error details

```
Error: expect(locator).toBeEnabled() failed

Locator: getByRole('button', { name: 'Lull', exact: true })
Expected: enabled
Timeout: 30000ms
Error: element(s) not found

Call log:
  - Expect "toBeEnabled" with timeout 30000ms
  - waiting for getByRole('button', { name: 'Lull', exact: true })

```

```yaml
- link "Skip to content":
  - /url: "#handbook-content"
- banner:
  - link "Conduit home":
    - /url: /conduit/
    - text: Conduit
  - navigation "Main navigation":
    - link "Home":
      - /url: /conduit/
    - link "Journeys":
      - /url: /conduit/journeys/
    - link "Handbook":
      - /url: /conduit/handbook/
    - link "Downloads":
      - /url: /conduit/#get-conduit
    - link "Open Conduit":
      - /url: /conduit/workspace/
  - link "Source":
    - /url: https://github.com/dancxjo/conduit
- complementary "Handbook navigation":
  - heading "Learn" [level=3]
  - list:
    - listitem:
      - link "Home":
        - /url: index.html
    - listitem:
      - link "Why Conduit":
        - /url: Why-Conduit.html
    - listitem:
      - link "Architecture tour":
        - /url: Architecture-tour.html
    - listitem:
      - link "Conduitese":
        - /url: Conduitese.html
    - listitem:
      - link "Conduitese by example":
        - /url: Conduitese-by-example.html
    - listitem:
      - link "Units and quantities":
        - /url: Units-and-quantities.html
    - listitem:
      - link "Feature example index":
        - /url: Conduitese-feature-coverage.html
    - listitem:
      - link "Plot diagrams":
        - /url: Plot-diagrams.html
    - listitem:
      - link "Glossary":
        - /url: Glossary.html
  - heading "Use" [level=3]
  - list:
    - listitem:
      - paragraph:
        - 'link "Creating models: teach Conduit to speak"':
          - /url: Creating-models.html
    - listitem:
      - paragraph:
        - link "Start here":
          - /url: Start-here.html
    - listitem:
      - paragraph:
        - link "Bodies, hosts, plans and plays":
          - /url: Bodies-hosts-plans-and-plays.html
    - listitem:
      - paragraph:
        - link "State, time and data":
          - /url: State-time-and-data.html
    - listitem:
      - paragraph:
        - link "Lines, networking and replanning":
          - /url: Lines-networking-and-replanning.html
    - listitem:
      - paragraph:
        - link "Face, mask and show":
          - /url: Face-Mask-and-Show.html
    - listitem:
      - paragraph:
        - link "ConduitOS and real machines":
          - /url: ConduitOS-and-real-machines.html
  - heading "Reference" [level=3]
  - list:
    - listitem:
      - link "Current language surface":
        - /url: Current-language-surface.html
    - listitem:
      - link "Architecture":
        - /url: Architecture.html
    - listitem:
      - link "Plots and flow":
        - /url: Plots-and-flow.html
    - listitem:
      - link "Types and state":
        - /url: Types-and-state.html
    - listitem:
      - link "Terminals and concurrency":
        - /url: Terminals-and-concurrency.html
    - listitem:
      - link "Effects and realization":
        - /url: Effects-and-realization.html
    - listitem:
      - link "Packs, hosts and bodies":
        - /url: Packs-hosts-and-bodies.html
  - heading "Status and governance" [level=3]
  - list:
    - listitem:
      - link "Evidence and proof":
        - /url: Evidence-and-proof.html
    - listitem:
      - link "Project status and roadmap":
        - /url: Project-status-and-roadmap.html
    - listitem:
      - link "Canon governance":
        - /url: Canon-governance.html
- main:
  - paragraph: Section 11 of 26
  - heading "Start here" [level=1]
  - paragraph:
    - text: Start with the
    - link "interactive Handbook":
      - /url: index.html
    - text: . You do not need to install Rust or run a server to use the published site. Your browser creates a local Conduit body for your examples, or recovers the one it already keeps.
  - heading "1. Predict what the clock will do" [level=2]
  - paragraph:
    - text: In the Handbook workbench, choose
    - strong: A clock you can stop
    - text: ". Read its source:"
    - code: time/every(1s)
    - text: produces ticks at an interval, and
    - code: ">>"
    - text: connects them to
    - code: presentation/tick
    - text: .
  - paragraph:
    - text: What would happen if you changed
    - code: 1s
    - text: to
    - code: 2s
    - text: "? Make a prediction before running it."
  - heading "2. Try it, then change it" [level=2]
  - paragraph:
    - text: Choose
    - strong: Try in my Handbook
    - text: . Watch the ticks in the running example.
  - paragraph:
    - text: Change
    - code: 1s
    - text: to
    - code: 2s
    - text: in the source editor, then choose
    - strong: Try in my Handbook
    - text: again. Compare the pace. The editor highlights your source while you type; checking and execution come from the Conduit runtime. If a change is refused, read the refusal and correct the source before trying again.
  - heading "3. Look inside your running example" [level=2]
  - paragraph:
    - text: Choose
    - strong: Open in Patchbay
    - text: ", then select the clock from the resident plot list. The diagram comes from the actual plot in your body."
  - paragraph: Follow the cord from the clock's output to the display's input. Select a gear, port, or cord to inspect its facts. Port labels tell you what kind of information can cross that connection. You can also use Tab and Enter to select subjects.
  - paragraph:
    - text: A
    - strong: plot
    - text: describes the work. A
    - strong: plan
    - text: selects how it can run. A
    - strong: play
    - text: is its execution. Expand
    - strong: Inspect exact evidence
    - text: when you want to inspect their identities; you do not need to memorize them to use the example.
  - heading "4. Stop, return, and continue" [level=2]
  - paragraph:
    - text: Choose
    - strong: Return to example
    - text: to see the clock again.
    - strong: Lull
    - text: stops its current execution;
    - strong: Wake
    - text: starts another run.
  - paragraph: Reload the Handbook. In the same browser storage, your local body and installed examples are recovered. The browser starts a fresh boot and admits fresh execution; it does not restore the old play as if it were still running.
  - paragraph:
    - strong: Your body and browser
    - text: shows storage status and offers
    - strong: Start my Handbook over
    - text: . Starting over forgets this application's body and examples while keeping the browser host identity. Another browser has its own body. Clearing local application storage can lose yours.
  - 'heading "Next: turn keystrokes into text" [level=2]'
  - paragraph:
    - text: Choose
    - strong: Turn keystrokes into text
    - text: ", try it, and focus the running example before typing. Open Patchbay to follow the keyboard input through the plot to visible text. Memory Lantern does not promise to retain your typed message across reload, even though the Handbook body itself has local continuity."
  - paragraph: "Continue with:"
  - list:
    - listitem:
      - link "Conduitese by example":
        - /url: Conduitese-by-example.html
      - text: to read and write plots.
    - listitem:
      - link "Bodies, hosts, plans, and plays":
        - /url: Bodies-hosts-plans-and-plays.html
      - text: to understand the identities you inspected.
    - listitem:
      - link "Glossary":
        - /url: Glossary.html
      - text: when a word is unfamiliar.
    - listitem:
      - link "Evidence and proof":
        - /url: Evidence-and-proof.html
      - text: to distinguish browser execution, recorded demonstrations, emulation, and physical operation.
  - 'heading "Optional: run from a checkout" [level=2]'
  - paragraph:
    - text: For repository work, follow the
    - link "contributor setup":
      - /url: https://github.com/dancxjo/conduit/blob/dev/CONTRIBUTING.md
    - text: ", then run the supported hosted entrance:"
  - code: cargo xtask make host std
  - paragraph: It checks, plans, and executes the Hello plot through the production kernel. This is an optional development workflow; the published Handbook does not need it to run your browser examples.
  - navigation "Continue reading":
    - text: Continue reading
    - link "Bodies hosts plans and plays":
      - /url: Bodies-hosts-plans-and-plays.html
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
  8   |   // Chromium's inspector can collect its unresolved evaluation promise even
  9   |   // while the actual snapshot remains live. Observe one real call's completion;
  10  |   // polling this record neither repeats the snapshot nor performs an action.
  11  |   await page.evaluate(() => {
  12  |     if (globalThis.__conduitProofSnapshot) throw new Error("Concurrent snapshot observation");
  13  |     const observation = { settled: false };
  14  |     globalThis.__conduitProofSnapshot = observation;
  15  |     globalThis.__conduitApplication.snapshot().then(value => {
  16  |       observation.value = value;
  17  |       observation.settled = true;
  18  |     }, error => {
  19  |       observation.error = { message: error.message, code: error.code };
  20  |       observation.settled = true;
  21  |     });
  22  |   });
  23  |   try {
  24  |     await page.waitForFunction(() => globalThis.__conduitProofSnapshot.settled, null, { timeout: 15_000 });
  25  |     return await page.evaluate(() => {
  26  |       const observation = globalThis.__conduitProofSnapshot;
  27  |       if (observation.error) throw Object.assign(new Error(observation.error.message), { code: observation.error.code });
  28  |       return observation.value;
  29  |     });
  30  |   } finally {
  31  |     await page.evaluate(() => { delete globalThis.__conduitProofSnapshot; });
  32  |   }
  33  | }
  34  | 
  35  | async function ready(page) {
  36  |   // The retained Candidate trace reached a real Playing Body after the default
  37  |   // five-second assertion budget expired during cold SDK/WASM acquisition.
> 38  |   await expect(page.getByRole("button", { name: "Lull", exact: true })).toBeEnabled({ timeout: 30_000 });
      |                                                                         ^ Error: expect(locator).toBeEnabled() failed
  39  |   await page.waitForFunction(() => Boolean(globalThis.__conduitApplication));
  40  |   const state = await current(page);
  41  |   for (const key of ["hostId", "bootId", "bodyId", "planId", "playId"]) expect(state[key], key).toBeTruthy();
  42  |   return state;
  43  | }
  44  | 
  45  | function renewed(first, next) {
  46  |   expect(next.hostId).toBe(first.hostId);
  47  |   expect(next.bodyId).toBe(first.bodyId);
  48  |   expect(next.bootId).not.toBe(first.bootId);
  49  |   expect(next.planId).not.toBe(first.planId);
  50  |   expect(next.playId).not.toBe(first.playId);
  51  | }
  52  | 
  53  | test("static Handbook applications retain independent local Bodies through use, navigation, exclusion, reset and browser restart", async ({}, testInfo) => {
  54  |   testInfo.setTimeout(180_000);
  55  |   const journey = await beginHandbookJourney();
  56  |   const site = await stageStaticApplications();
  57  |   const origin = new URL(site.url).origin;
  58  |   const application = new URL("handbook/", site.url).href;
  59  |   const otherApplication = new URL("second/", site.url).href;
  60  |   const profile = testInfo.outputPath("retained-browser-profile");
  61  |   let session;
  62  |   let retained;
  63  |   try {
  64  |     session = await openStaticProfile(profile, origin);
  65  |     const page = await session.context.newPage();
  66  |     await page.goto(application);
  67  |     const first = await ready(page);
  68  |     const opening = await page.evaluate(() => globalThis.__conduitApplication.opening);
  69  |     expect(opening.recovered).toBe(false);
  70  |     expect(opening.birthState).toBe("Lulled");
  71  |     await journey.capture(page, { id: "open", title: "Open your own Handbook",
  72  |       action: "Open the static Handbook in a fresh browser profile.",
  73  |       observation: "The Handbook creates a local Body, then wakes its resident application. The page offers lessons, editable source, and lifecycle controls." });
  74  |     await page.screenshot({ path: testInfo.outputPath("01-handbook-first-open.png"), fullPage: true });
  75  | 
  76  |     await page.getByRole("link", { name: /^Find your bearings/ }).click();
  77  |     await page.getByRole("heading", { name: "Start here", exact: true }).waitFor();
  78  |     expect((await ready(page)).bodyId).toBe(first.bodyId);
  79  |     await page.getByLabel("Choose an example", { exact: true }).selectOption({ label: "A ticker you can stop" });
  80  |     await expect(page.getByRole("button", { name: "Try in my Handbook", exact: true })).toBeEnabled();
  81  |     const editor = page.getByRole("textbox", { name: "Plot source", exact: true });
  82  |     const originalSource = await editor.inputValue();
  83  |     const highlighting = page.locator(".syntax-highlight code");
  84  |     await expect(highlighting).toHaveText(originalSource, { useInnerText: false });
  85  |     const initialTokens = await highlighting.locator("span").count();
  86  |     expect(initialTokens).toBeGreaterThan(0);
  87  |     const incomplete = "plot café {\n    # λ learning\n    clock: time/every(";
  88  |     await editor.fill(incomplete);
  89  |     await expect(highlighting).toHaveText(incomplete, { useInnerText: false });
  90  |     await expect(editor).toHaveAttribute("data-syntax-disposition", "accepted");
  91  |     expect(await highlighting.locator("span").count()).not.toBe(initialTokens);
  92  |     const beforeRefusal = await current(page);
  93  |     await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
  94  |     await expect(page.locator("[data-check]")).not.toHaveText("Edit the source, then check and try it in your body.");
  95  |     await expect(page.locator("[data-check]")).toHaveAttribute("data-refusal-code", /.+/);
  96  |     const afterRefusal = await current(page);
  97  |     expect(afterRefusal.bodyId).toBe(beforeRefusal.bodyId);
  98  |     expect(afterRefusal.playId).toBe(beforeRefusal.playId);
  99  |     expect(afterRefusal.installedPlots).toEqual(beforeRefusal.installedPlots);
  100 |     await journey.capture(page, { id: "check", title: "Check an unfinished edit",
  101 |       action: "Open Start here, choose the clock lesson, enter unfinished source, and press Try in my Handbook.",
  102 |       observation: "Live highlighting follows the Unicode edit. The checker reports a refusal; the existing Body, Play, and installed workset remain unchanged." });
  103 |     await editor.fill(originalSource);
  104 |     await page.getByRole("button", { name: "Try in my Handbook", exact: true }).click();
  105 |     // Controls reenable after installation and the actual SDK current() settle.
  106 |     let example = await ready(page);
  107 |     await expect.poll(async () => { const state = await current(page); return state.installedPlots.some(plot => plot.checked_plot_id === state.selectedPlot); }).toBe(true);
  108 |     expect(example.bodyId).toBe(first.bodyId);
  109 |     expect(example.selectedPlot).toBeTruthy();
  110 |     expect(example.installedPlots.some(plot => plot.checked_plot_id === example.selectedPlot)).toBe(true);
  111 |     await journey.capture(page, { id: "try", title: "Run the clock lesson",
  112 |       action: "Restore the clock source and press Try in my Handbook.",
  113 |       observation: "The checked clock joins the same Body and runs through a newly admitted Plan and Play." });
  114 |     await page.getByRole("button", { name: "Open in Patchbay", exact: true }).first().click();
  115 |     await page.locator(".handbook-show").getByRole("button", { name: /clock-demo|A ticker you can stop/i }).click();
  116 |     const graph = page.locator(".handbook-show [data-checked-plot-id]:visible");
  117 |     await expect(graph).toHaveAttribute("data-checked-plot-id", example.selectedPlot);
  118 |     await expect(graph.getByRole("group", { name: "Resident plot gears, ports, and cords" })).toBeVisible();
  119 |     await expect(graph).toHaveAttribute("data-body-plan-id", example.planId);
  120 |     await expect(graph).toHaveAttribute("data-active-play-id", example.playId);
  121 |     const outputPort = graph.getByRole("button", { name: /^output tick,/ });
  122 |     await outputPort.focus();
  123 |     await outputPort.press("Enter");
  124 |     await expect(outputPort).toHaveAttribute("aria-pressed", "true");
  125 |     await expect(outputPort).toBeFocused();
  126 |     let inspected = await current(page);
  127 |     expect(inspected.bodyId).toBe(first.bodyId);
  128 |     expect(inspected.selectedPlot).toEqual(example.selectedPlot);
  129 |     expect(inspected.foreground.checked_plot_id).not.toBe(example.selectedPlot);
  130 |     await journey.capture(page, { id: "inspect", title: "Inspect the running clock",
  131 |       action: "Press Open in Patchbay, choose the clock, focus its output port, and press Enter.",
  132 |       observation: "The resident Patchbay shows the clock's gears, ports, and cord. Its selected output port and graph identities belong to this Body's actual Plan and Play." });
  133 |     await page.screenshot({ path: testInfo.outputPath("02-real-patchbay.png"), fullPage: true });
  134 | 
  135 |     const originalExample = example;
  136 |     const editedSource = originalSource.replace("time/every(1s)", "time/every(2s)");
  137 |     expect(editedSource).not.toBe(originalSource);
  138 |     await editor.fill(editedSource);
```