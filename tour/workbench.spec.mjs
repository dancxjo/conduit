import { expect, test } from "@playwright/test";

test.describe.configure({ timeout: 90_000 });

async function gotoWorkbench(page) {
  await page.goto("/tour/public/workbench.html");
  await expect(page.locator("html")).toHaveAttribute("data-workbench-ready", "true", {
    timeout: 30_000,
  });
}

async function filterPalette(page, query) {
  await page.locator("#palette-search").fill(query);
  return page.locator(".palette-item", { hasText: query }).first();
}

async function connect(page, fromNode, fromPort, toNode, toPort) {
  await page.locator("#connection-from").selectOption(`${fromNode}::${fromPort}`);
  await page.locator("#connection-to").selectOption(`${toNode}::${toPort}`);
  await page.locator("#connect-ports").click();
}

test("Workbench authors, runs, saves, reopens, and round-trips one ordinary graph", async ({ page }) => {
  await gotoWorkbench(page);
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(0);

  const literal = await filterPalette(page, "std/literal");
  await literal.dragTo(page.locator("#cy"), { targetPosition: { x: 460, y: 360 } });
  await expect(page.locator("#source")).toHaveValue(/literal: std\/literal/);

  const upper = await filterPalette(page, "text/uppercase");
  await upper.getByRole("button", { name: /Add text\/uppercase/ }).click();
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(2, { timeout: 15_000 });
  const display = await filterPalette(page, "display/text");
  await display.getByRole("button", { name: /Add display\/text/ }).click();
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(3, { timeout: 15_000 });

  await page.locator('[data-panel="palette"] [data-panel-collapse-control]').click();
  await page.locator('[data-panel="inspector"] [data-panel-collapse-control]').click();
  await page.locator('.react-flow__node[data-id="literal"]').click();
  await page.locator("#node-config input").fill("Workbench says hello.\n");
  await page.locator("#node-config button", { hasText: "Set value" }).click();
  await expect(page.locator("#workbench-status")).toContainText(/committed/i, { timeout: 10_000 });

  await connect(page, "literal", "value", "uppercase", "text");
  await connect(page, "uppercase", "text", "text", "text");
  await expect(page.locator("#source")).toHaveValue(/literal\.value > uppercase\.text/);
  await expect(page.locator("#source")).toHaveValue(/uppercase\.text > text\.text/);
  await expect(page.locator("#run")).toBeEnabled();

  await page.locator("#run").click();
  await expect(page.locator("#run-result")).toContainText("WORKBENCH SAYS HELLO.", {
    timeout: 20_000,
  });
  await expect(page.locator("#evidence")).not.toHaveText("No evidence yet.");

  page.once("dialog", (dialog) => dialog.accept("vertical-slice"));
  await page.locator("#save-document").click();
  await page.locator("#new-document").click();
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(0);
  await page.locator("#saved-documents").selectOption("vertical-slice");
  await page.locator("#open-document").click();
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(3);
  await expect(page.locator("#source")).toHaveValue(/Workbench says hello/);

  await page.locator('[data-panel="source"] [data-panel-collapse-control]').click();
  await expect(page.locator("#source")).toBeVisible();
  await page.locator("#source").evaluate((element) => {
    element.value = element.value.replace("Workbench says hello.", "Source says hello.");
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await expect(page.locator("#source")).toHaveValue(/Source says hello/);

  await page.locator("#undo").click();
  await expect(page.locator("#redo")).toBeEnabled({ timeout: 15_000 });
  await page.locator("#redo").click();
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(3);
});

test("Workbench preserves the viewport when a node is dropped", async ({ page }) => {
  await gotoWorkbench(page);

  const literal = await filterPalette(page, "std/literal");
  await literal.getByRole("button", { name: /Add std\/literal/ }).click();
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(1);

  const viewport = page.locator("#cy .react-flow__viewport");
  const transformBeforeZoom = await viewport.evaluate((element) => element.style.transform);
  await page.locator("#cy").hover({ position: { x: 640, y: 360 } });
  await page.mouse.wheel(0, -600);
  await expect.poll(() => viewport.evaluate((element) => element.style.transform))
    .not.toBe(transformBeforeZoom);
  const transformBeforeDrop = await viewport.evaluate((element) => element.style.transform);

  const upper = await filterPalette(page, "text/uppercase");
  const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
  await upper.dispatchEvent("dragstart", { dataTransfer });
  const canvasBounds = await page.locator("#cy").boundingBox();
  await page.locator("#workbench-canvas").dispatchEvent("drop", {
    dataTransfer,
    clientX: canvasBounds.x + canvasBounds.width / 2,
    clientY: canvasBounds.y + canvasBounds.height / 2,
  });
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(2);
  await expect.poll(() => viewport.evaluate((element) => element.style.transform))
    .toBe(transformBeforeDrop);
});

test("Workbench exposes honest unsupported palette entries and remains usable narrow at 200 percent", async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 800 });
  await gotoWorkbench(page);
  const canvasBounds = await page.locator("#cy").boundingBox();
  expect(canvasBounds.x).toBe(0);
  expect(canvasBounds.y).toBe(0);
  expect(canvasBounds.width).toBe(640);
  expect(canvasBounds.height).toBe(800);
  await page.evaluate(() => { document.documentElement.style.zoom = "2"; });
  const unsupported = page.locator(".palette-status[data-supported=false]").first();
  await expect(unsupported).toContainText(/Unavailable here · CND-/);
  await expect(page.locator("#palette-search")).toBeVisible();
  await expect(page.locator("#cy")).toBeVisible();
  await expect(page.locator('[data-panel="source"]')).toHaveAttribute("data-panel-collapsed", "true");

  const item = await filterPalette(page, "std/literal");
  await item.getByRole("button", { name: /Add std\/literal/ }).focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(1);

  const stdout = await filterPalette(page, "io/stdout");
  await stdout.getByRole("button", { name: /Add io\/stdout/ }).click();
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(2);

  const palettePanel = page.locator('[data-panel="palette"]');
  await palettePanel.locator("[data-panel-collapse-control]").click();
  await expect(palettePanel).toHaveAttribute("data-panel-collapsed", "true");
  const inspectorPanel = page.locator('[data-panel="inspector"]');
  await expect(inspectorPanel).toHaveAttribute("data-panel-collapsed", "true");
  await inspectorPanel.locator("[data-panel-collapse-control]").click();
  await expect(inspectorPanel).toHaveAttribute("data-panel-collapsed", "false");
  await connect(page, "literal", "value", "stdout", "bytes");
  await expect(page.locator("#workbench-status")).toContainText("CND-TYP-001");
  await expect(page.locator("#source")).not.toHaveValue(/literal\.value > stdout\.bytes/);

  await inspectorPanel.locator("[data-panel-collapse-control]").click();
  await expect(inspectorPanel).toHaveAttribute("data-panel-collapsed", "true");
  const sourcePanel = page.locator('[data-panel="source"]');
  await expect(sourcePanel).toHaveAttribute("data-panel-collapsed", "true");
  await sourcePanel.locator("[data-panel-mode-control]").click();
  await expect(sourcePanel).toHaveAttribute("data-panel-mode", "floating");
  await sourcePanel.locator("[data-panel-mode-control]").click();
  await expect(sourcePanel).toHaveAttribute("data-panel-mode", "docked");
});
