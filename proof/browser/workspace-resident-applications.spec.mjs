import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;
const current = page => page.evaluate(() => globalThis.__conduitWorkspace.current());
const visibleFormOutput = page => page.locator("[data-form-output] > output:not([hidden])");

test.beforeEach(async ({ page }) => {
  entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
  await page.setViewportSize({ width: 1280, height: 1000 });
});
test.afterEach(() => entrance?.child.kill());

test("resident application styles are package-bound, scoped, and responsive", async ({ page }, testInfo) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  for (const title of ["Tutorial", "Patchbay"]) {
    await page.getByRole("checkbox", { name: title, exact: true }).check();
  }
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "Wake Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await expect(page.locator('style[data-application-resource="resident-tour-style"]')).toHaveCount(1);

  const packagedStyle = await page.evaluate(async () => {
    const manifest = await fetch("./workspace.application.json").then(response => response.json());
    return manifest.resources.find(resource => resource.role === "resident-tour-style");
  });
  expect(packagedStyle).toMatchObject({ kind: "style", path: "resident-tour.css", maximum_bytes: 8192 });
  expect(packagedStyle.sha256).toMatch(/^sha256:[0-9a-f]{64}$/);

  await page.locator("[data-checked-form-id]").filter({ hasText: "Tutorial" }).click();
  const tour = visibleFormOutput(page);
  await expect(tour).toHaveAttribute("data-presentation-kind", "presentation/application-view");
  await expect(tour.locator(':scope > [data-application-key="tour"]')).toHaveCSS("display", "grid");
  await expect(tour).toHaveCSS("font-size", "16px");
  const source = tour.locator('[data-application-key="source"]');
  await expect(source).toHaveCSS("font-size", "13.6px");
  await expect(tour.locator('[data-application-key="result"]')).toContainText("Not run");
  await expect(tour.locator('[data-application-key="result"]')).toBeVisible();
  expect((await source.evaluate(element => getComputedStyle(element).fontFamily)).toLowerCase()).toContain("mono");
  const desktop = await tour.evaluate(element => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
  }));
  expect(desktop.scrollWidth).toBeLessThanOrEqual(desktop.clientWidth);
  expect(desktop.clientHeight).toBeLessThanOrEqual(704);
  expect(desktop.scrollHeight).toBeLessThan(1400);
  await page.screenshot({ path: testInfo.outputPath("resident-tour-styled.png"), fullPage: true });

  await page.locator("[data-checked-form-id]").filter({ hasText: "Patchbay" }).click();
  const patchbay = visibleFormOutput(page);
  await expect(patchbay).toHaveAttribute("data-presentation-kind", "presentation/application-view");
  await expect(patchbay).toHaveCSS("font-size", "16px");
  await expect(patchbay.locator(':scope > [data-application-key="tour"]')).toHaveCount(0);

  await page.locator("[data-checked-form-id]").filter({ hasText: "Memory Lantern" }).click();
  await page.locator("#form-input").focus();
  await page.keyboard.type("large");
  const text = visibleFormOutput(page);
  await expect(text).toHaveAttribute("data-presentation-kind", "presentation/text");
  await expect(text).toHaveCSS("font-size", "56px");

  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator("[data-checked-form-id]").filter({ hasText: "Tutorial" }).click();
  const narrow = await visibleFormOutput(page).evaluate(element => {
    const lesson = element.querySelector('[data-application-key="lesson"]').getBoundingClientRect();
    const patchbay = element.querySelector('[data-application-key="patchbay-panel"]').getBoundingClientRect();
    return {
      documentWidth: document.documentElement.scrollWidth,
      outputClientWidth: element.clientWidth,
      outputScrollWidth: element.scrollWidth,
      outputWidth: element.getBoundingClientRect().width,
      lesson: { x: lesson.x, width: lesson.width },
      patchbay: { x: patchbay.x, width: patchbay.width },
    };
  });
  expect(narrow.documentWidth).toBeLessThanOrEqual(390);
  expect(narrow.outputScrollWidth).toBeLessThanOrEqual(narrow.outputClientWidth);
  expect(Math.abs(narrow.lesson.x - narrow.patchbay.x)).toBeLessThan(1);
  expect(Math.abs(narrow.lesson.width - narrow.patchbay.width)).toBeLessThan(1);
  await page.screenshot({ path: testInfo.outputPath("resident-tour-styled-narrow.png"), fullPage: true });
});

test("legacy resident Tutorial keeps state across foreground switches", async ({ page }, testInfo) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  for (const title of ["Tutorial", "Patchbay"]) {
    await page.getByRole("checkbox", { name: title, exact: true }).check();
  }
  await page.getByLabel("Friendly Body name", { exact: true }).fill("Wayfinder");
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "Wake Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const born = await current(page);
  expect(born.initial_forms).toHaveLength(3);

  await page.locator("[data-checked-form-id]").filter({ hasText: "Tutorial" }).click();
  const tour = visibleFormOutput(page);
  await expect(tour).toContainText("One Program, Many Computers");
  await page.getByRole("button", { name: "Run Form", exact: true }).click();
  await expect(tour).toContainText("HELLO");

  await page.getByRole("button", { name: "Next exercise", exact: true }).click();
  await expect(tour).toContainText("edit-one-gear");
  await page.getByRole("button", { name: "Run Form", exact: true }).click();
  await expect(tour).toContainText("MAKE THIS LOUD");

  await page.getByRole("button", { name: "Next exercise", exact: true }).click();
  await expect(tour).toContainText("branch-a-cord");
  await page.getByRole("button", { name: "Run Form", exact: true }).click();
  await expect(tour).toContainText("SOS");

  await page.getByRole("button", { name: "Next chapter", exact: true }).click();
  await expect(tour).toContainText("Faces, Backs, and implementation");
  await page.getByRole("button", { name: "Run Form", exact: true }).click();
  await expect(tour).toContainText("Direct and recursive realizations agree");
  await expect(tour).toContainText("distinct expanded Forms and Plans");

  await page.getByRole("button", { name: "Next chapter", exact: true }).click();
  await expect(tour).toContainText("Hosts make Forms real");
  await page.getByRole("button", { name: "Run Form", exact: true }).click();
  await expect(tour).toContainText("0 → 1");
  await expect(tour).toContainText("120 ms timer pending");

  await page.getByRole("button", { name: "Next chapter", exact: true }).click();
  await expect(tour).toContainText("One Form across several Hosts");
  await page.getByRole("button", { name: "Run Form", exact: true }).click();
  await expect(tour).toContainText("one value delivered over one planned Line");
  await page.getByRole("button", { name: "Next exercise", exact: true }).click();
  await page.getByRole("button", { name: "Run Form", exact: true }).click();
  await expect(tour).toContainText("one value delivered over one planned Line");

  for (const title of [
    "The Body: one computer, one machine or many",
    "Many Forms, one Body-wide realization",
    "Birth, spores, and the Crèche",
  ]) {
    await page.getByRole("button", { name: "Next chapter", exact: true }).click();
    await expect(tour).toContainText(title);
    await expect(page.getByRole("button", { name: "Run Form", exact: true })).toHaveCount(0);
  }
  await page.screenshot({ path: testInfo.outputPath("resident-tour-result.png"), fullPage: true });

  await page.locator("[data-checked-form-id]").filter({ hasText: "Memory Lantern" }).click();
  await page.locator("#form-input").focus();
  await page.keyboard.type("kept");
  await expect(visibleFormOutput(page)).toHaveText("kept");
  await page.locator("[data-checked-form-id]").filter({ hasText: "Tutorial" }).click();
  await expect(visibleFormOutput(page)).toContainText("Birth, spores, and the Crèche");

  await page.locator("[data-checked-form-id]").filter({ hasText: "Patchbay" }).click();
  const patchbay = visibleFormOutput(page);
  await expect(patchbay).toContainText("Active Forms on this Body");
  await expect(patchbay).toContainText("Memory Lantern");
  await expect(patchbay).toContainText("Tutorial");
  await expect(patchbay).toContainText("Patchbay");
  await expect(patchbay).toContainText("Playing");
  await expect(patchbay).toContainText("foreground");
  await patchbay.getByRole("button", { name: "Tutorial", exact: true }).click();
  await expect(patchbay).toContainText("tour/events");
  await expect(patchbay).toContainText("Body Plan");
  await page.getByRole("button", { name: "Inspect next", exact: true }).click();
  await expect(patchbay.locator('[data-application-key="identity"]')).toContainText("Plan");
  await page.getByRole("button", { name: "Edit current", exact: true }).click();
  await page.screenshot({ path: testInfo.outputPath("resident-patchbay-inspection.png"), fullPage: true });

  const after = await current(page);
  expect(after.body_id).toBe(born.body_id);
  expect(after.plan_id).toBe(born.plan_id);
  expect(after.active_play_id).toBe(born.active_play_id);
  await page.getByRole("button", { name: "Lull Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
});

test("resident Patchbay inspects and begins an exact edit in one browser Body Play", async ({ page }, testInfo) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Patchbay", exact: true }).check();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "Wake Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const born = await current(page);
  await page.locator("[data-checked-form-id]").filter({ hasText: "Patchbay" }).click();
  const patchbay = visibleFormOutput(page);
  await expect(patchbay).toContainText("Active Forms on this Body");
  await patchbay.getByRole("button", { name: "Memory Lantern", exact: true }).click();
  await expect(patchbay).toContainText("memory_lantern");
  await expect(patchbay).toContainText("Body Plan");
  await page.getByRole("button", { name: "Inspect next", exact: true }).click();
  await page.getByRole("button", { name: "Edit current", exact: true }).click();
  await expect(patchbay.locator('[data-application-key="edit-request"]')).toContainText("no authority was inferred");
  expect((await current(page)).active_play_id).toBe(born.active_play_id);
});
