import { expect, test } from "@playwright/test";

test("runs a production lesson in the resolved browser worker", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });

  await page.goto("/tour/public/index.html?autorun");
  await expect(page.locator("#result")).toContainText("Hello from the Tour.", {
    timeout: 20_000,
  });
  await expect(page.locator("#result")).toContainText(
    "Evidence: 2 nodes, 1 cords conducted.",
  );
  await expect(page.locator("#execution-note")).toContainText(
    "exact dedicated-worker placement",
  );
  await expect(page.locator("#plan")).toContainText(
    "conduit/tour-production-wasm-worker",
  );
  await expect(page.locator("#evidence")).toContainText("lesson-completed");
  expect(failures).toEqual([]);
});

test("preserves a recoverable draft across reset", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.fill((await source.inputValue()).replace("Hello from the Tour.", "Recover me."));
  await page.locator("#reset").click();
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await page.locator("#undo-reset").click();
  await expect(source).toHaveValue(/Recover me\./);
});

test("covers Chapters 0-3 and exposes production topology projections", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await expect(page.locator("#lessons > li")).toHaveCount(15);
  await page.getByRole("button", { name: "Inside / outside" }).click();
  await expect(page.locator("#source")).toHaveValue(/example\/upper-box/);
  await page.locator("#expanded-view").click();
  await expect(page.locator("#topology")).toContainText(
    "box.worker : conduit.std/uppercase",
  );
  await page.locator("#logical-view").click();
  await expect(page.locator("#topology")).toContainText(
    "composite box : example/upper-box",
  );
});

test("accepts a semantically correct alternate solution", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.fill(
    (await source.inputValue())
      .replace("node greeting ", "node salutation ")
      .replace("greeting.out", "salutation.out"),
  );
  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("✓ Lesson complete!", {
    timeout: 20_000,
  });
  await expect(source).toHaveValue(/node salutation/);
});

test("routes projected cords around intervening faceplates", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const route = await page.evaluate(async () => {
    const { routeAroundNodes } = await import("./patchbay-smart-edge.js");
    return routeAroundNodes(
      { x: 0, y: 80 },
      { x: 320, y: 80 },
      [{
        id: "middle",
        positionAbsolute: { x: 120, y: 32 },
        width: 80,
        height: 96,
      }],
    );
  });

  expect(route).not.toBeNull();
  expect(route.path).toMatch(/^M /);
  expect(route.points.some((point) => point.y < 16 || point.y > 144)).toBe(true);
  expect(route.points.every((point) =>
    point.x < 104 || point.x > 216 || point.y < 16 || point.y > 144
  )).toBe(true);
});
