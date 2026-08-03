import { expect, test } from "@playwright/test";

test.describe.configure({ timeout: 90_000 });

const desktopViewports = [
  { width: 1920, height: 1080 },
  { width: 1440, height: 900 },
  { width: 1280, height: 720 },
];

async function gotoWorkbench(page, viewport) {
  await page.setViewportSize(viewport);
  await page.goto("/tour/public/workbench.html");
  await expect(page.locator("html")).toHaveAttribute(
    "data-workbench-ready",
    "true",
    {
      timeout: 30_000,
    },
  );
}

async function expectInsideViewport(locator, viewport) {
  const bounds = await locator.boundingBox();
  expect(bounds).not.toBeNull();
  expect(bounds.x).toBeGreaterThanOrEqual(0);
  expect(bounds.y).toBeGreaterThanOrEqual(0);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(viewport.width + 1);
  expect(bounds.y + bounds.height).toBeLessThanOrEqual(viewport.height + 1);
}

async function expectNoHorizontalOverflow(locator) {
  const overflow = await locator.evaluate((element) => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
  }));
  expect(overflow.scrollWidth).toBeLessThanOrEqual(overflow.clientWidth + 1);
}

async function expectCompactHeader(panel) {
  const header = panel.locator(":scope > .workbench-panel-header");
  const heading = header.locator(":scope > .workbench-panel-heading");
  const actions = header.locator(":scope > .workbench-panel-controls");
  await expect(heading).toBeVisible();
  await expect(actions).toBeVisible();
  const headingBounds = await heading.boundingBox();
  const actionBounds = await actions.boundingBox();
  expect(headingBounds.x + headingBounds.width).toBeLessThanOrEqual(
    actionBounds.x + 1,
  );
}

for (const viewport of desktopViewports) {
  test(`Workbench stays dense without horizontal overflow at ${viewport.width}x${viewport.height}`, async ({ page }) => {
    await gotoWorkbench(page, viewport);

    const palette = page.locator('[data-panel="palette"]');
    const source = page.locator('[data-panel="source"]');
    const inspector = page.locator('[data-panel="inspector"]');
    await source.locator("[data-panel-collapse-control]").click();
    await inspector.locator("[data-panel-collapse-control]").click();

    for (const panel of [palette, source, inspector]) {
      await expectInsideViewport(panel, viewport);
      await expectNoHorizontalOverflow(panel);
      await expectCompactHeader(panel);
    }

    for (
      const control of [
        "#new-document",
        "#saved-documents",
        "#open-document",
        "#save-document",
        "#undo",
        "#redo",
        "#run",
        "#stop",
      ]
    ) {
      await expectInsideViewport(page.locator(control), viewport);
    }

    const paletteBounds = await palette.boundingBox();
    const inspectorBounds = await inspector.boundingBox();
    const workingLaneWidth = inspectorBounds.x -
      (paletteBounds.x + paletteBounds.width);
    expect(workingLaneWidth).toBeGreaterThan(paletteBounds.width);

    const canvasTitleSize = await page.locator("#canvas-title").evaluate((
      element,
    ) => Number.parseFloat(getComputedStyle(element).fontSize));
    const paletteTitleSize = await page.locator("#palette-title").evaluate((
      element,
    ) => Number.parseFloat(getComputedStyle(element).fontSize));
    expect(canvasTitleSize).toBeGreaterThanOrEqual(26);
    expect(canvasTitleSize).toBeLessThanOrEqual(32);
    expect(paletteTitleSize).toBeGreaterThanOrEqual(18);
    expect(paletteTitleSize).toBeLessThanOrEqual(22);

    const longestPaletteTitle = await page.locator(".palette-item h3")
      .evaluateAll((titles) => {
        const longest = titles.reduce((candidate, title) =>
          title.textContent.length > candidate.textContent.length
            ? title
            : candidate
        );
        return {
          clientWidth: longest.clientWidth,
          scrollWidth: longest.scrollWidth,
        };
      });
    expect(longestPaletteTitle.scrollWidth).toBeLessThanOrEqual(
      longestPaletteTitle.clientWidth + 1,
    );
    await expectNoHorizontalOverflow(page.locator("html"));
  });
}

test("Workbench uses bounded side drawers around a primary canvas at 1024px", async ({ page }) => {
  const viewport = { width: 1024, height: 768 };
  await gotoWorkbench(page, viewport);

  const canvasBounds = await page.locator("#cy").boundingBox();
  expect(canvasBounds).toEqual({ x: 0, y: 0, width: 1024, height: 768 });

  const palette = page.locator('[data-panel="palette"]');
  const inspector = page.locator('[data-panel="inspector"]');
  const source = page.locator('[data-panel="source"]');
  for (const panel of [palette, inspector, source]) {
    await expectInsideViewport(panel, viewport);
    await expectCompactHeader(panel);
  }

  const paletteBounds = await palette.boundingBox();
  expect(paletteBounds.width).toBeGreaterThanOrEqual(280);
  expect(paletteBounds.width).toBeLessThanOrEqual(340);
  await expectNoHorizontalOverflow(palette);

  await palette.locator("[data-panel-collapse-control]").click();
  await inspector.locator("[data-panel-collapse-control]").click();
  await expectNoHorizontalOverflow(inspector);
  await inspector.locator("[data-panel-collapse-control]").click();
  await source.locator("[data-panel-collapse-control]").click();
  await expectNoHorizontalOverflow(source);

  for (
    const control of [
      "#new-document",
      "#saved-documents",
      "#open-document",
      "#save-document",
      "#undo",
      "#redo",
      "#run",
      "#stop",
    ]
  ) {
    await expectInsideViewport(page.locator(control), viewport);
  }
  await expectNoHorizontalOverflow(page.locator("html"));
});
