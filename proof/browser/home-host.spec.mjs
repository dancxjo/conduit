import { expect, test } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { startStaticProduct } from "./tour-test-server.mjs";

const LOCAL_STEPS = [
  "home.arrived", "forms.opened", "form.selected", "prompt.opened",
  "form.run", "play.observed",
];

let entrance;

test.beforeAll(async () => { entrance = await startStaticProduct("target/home-product", "/home/"); });
test.afterAll(() => entrance?.child.kill());

test("portable Home journey runs through the shared model", async ({ page }, testInfo) => {
  await page.goto(entrance.url);
  await expect(page.locator("html")).toHaveAttribute("data-conduit-load-state", "ready");
  await expect(page.locator("#host-state")).toHaveText("Browser Home is ready.");
  await expect(page.getByRole("button", { name: "FORMS" })).toBeVisible();

  await page.getByRole("button", { name: "FORMS" }).click();
  await expect(page.getByRole("button", { name: "Text Lab" })).toBeVisible();
  await page.getByRole("button", { name: "Text Lab" }).click();
  await expect(page.locator("#host-state")).toHaveAttribute("data-request", "4");

  const command = page.getByLabel("conduit>");
  await command.fill("open prompt");
  await command.press("Enter");
  await expect(page.getByText("Conduit Prompt")).toBeVisible();
  await command.fill("run hello");
  await command.press("Enter");
  await expect(page.locator("#host-state")).toHaveText("HELLO, WORLD.");
  await expect(page.locator("#host-state")).toHaveAttribute("data-play-disposition", "completed");

  await command.fill("home");
  await command.press("Enter");
  await expect(page.getByRole("button", { name: "TOUR" })).toBeVisible();

  await expect(page.locator("html")).toHaveAttribute("data-home-steps", LOCAL_STEPS.join(","));
  expect(["chromium", "firefox"]).toContain(testInfo.project.name);
});

test("Home package binds its exact browser runtime", async ({ request }) => {
  const response = await request.get(new URL("home.application.json", entrance.url).href);
  expect(response.ok()).toBeTruthy();
  const manifest = await response.json();
  expect(manifest.application_id).toBe("conduit.application/home");
  expect(manifest.resources.find((resource) => resource.role === "runtime")?.sha256).toMatch(/^sha256:[0-9a-f]{64}$/);
});

test("Home front has keyboard and WCAG 2.2 AA structure", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Home is ready.");
  const skip = page.getByRole("link", { name: "Skip to main content" });
  await skip.focus();
  await skip.press("Enter");
  await expect(page.getByRole("main")).toBeFocused();
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();
  expect(results.violations, JSON.stringify(results.violations, null, 2)).toEqual([]);
});
