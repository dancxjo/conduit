import { expect, test } from "@playwright/test";

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
  await literal.dragTo(page.locator("#cy"), { targetPosition: { x: 150, y: 140 } });
  await expect(page.locator("#source")).toHaveValue(/literal: std\/literal/);

  const upper = await filterPalette(page, "text/uppercase");
  await upper.getByRole("button", { name: /Add text\/uppercase/ }).click();
  const display = await filterPalette(page, "display/text");
  await display.getByRole("button", { name: /Add display\/text/ }).click();
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(3);

  await page.locator("#source").evaluate((element) => {
    element.value = element.value.replace('value = ""', 'value = "Workbench says hello.\\n"');
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
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

  await page.locator("#undo").click();
  await expect(page.locator("#redo")).toBeEnabled();
  await page.locator("#redo").click();
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(3);
});

test("Workbench exposes honest unsupported palette entries and remains usable narrow at 200 percent", async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 800 });
  await gotoWorkbench(page);
  await page.evaluate(() => { document.documentElement.style.zoom = "2"; });
  const unsupported = page.locator(".palette-status[data-supported=false]").first();
  await expect(unsupported).toContainText(/Unavailable here · CND-/);
  await expect(page.locator("#palette-search")).toBeVisible();
  await expect(page.locator("#cy")).toBeVisible();
  await expect(page.locator("#source")).toBeVisible();

  const item = await filterPalette(page, "std/literal");
  await item.getByRole("button", { name: /Add std\/literal/ }).focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#cy .react-flow__node")).toHaveCount(1);
});
