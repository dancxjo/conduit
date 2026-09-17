import { openCrecheStep } from "./creche-test-actions.mjs";
import AxeBuilder from "@axe-core/playwright";
import { cp, mkdir, rm } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { stageLegacyTourRoutes } from "../../products/tour/tools/stage-legacy-routes.mjs";
import { startStaticProduct } from "./tour-test-server.mjs";

const pagesRoot = "target/web-accessibility-proof";
let entrance;

test.beforeAll(async () => {
  await rm(pagesRoot, { recursive: true, force: true });
  await mkdir(pagesRoot, { recursive: true });
  await cp("target/pages-root", pagesRoot, { recursive: true });
  await cp("target/tour-product", `${pagesRoot}/tour`, { recursive: true });
  await cp("target/creche-product", `${pagesRoot}/creche`, { recursive: true });
  await cp("target/workspace-product", `${pagesRoot}/workspace`, { recursive: true });
  await cp("target/patchbay-product", `${pagesRoot}/patchbay`, { recursive: true });
  await cp("target/home-product", `${pagesRoot}/home`, { recursive: true });
  await stageLegacyTourRoutes(pagesRoot);
  entrance = await startStaticProduct(pagesRoot, "/conduit/");
});

test.afterAll(() => entrance?.child.kill());

for (const [name, path] of [["Home", ""], ["Home face", "home/"], ["Body", "workspace/"], ["Crèche", "creche/"], ["Patchbay", "patchbay/"]]) {
  test(`${name} rendered entrance has WCAG 2.2 AA structure`, async ({ page }) => {
    await page.goto(`${entrance.url}${path}`);
    if (name === "Body") await expect(page.locator("[data-body-tutorial]")).toBeVisible();
    if (name === "Crèche") await expect(page.locator("#host-state")).toHaveText("Crèche ready");
    if (name === "Patchbay") await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    if (name === "Home face") await expect(page.locator("#host-state")).toHaveText("Browser Home is ready.");
    await expect(page.locator("#conduit-suspense")).toHaveCount(0);
    await expect(page.getByRole("main")).toHaveCount(1);
    await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
    const results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
    expect(results.violations, JSON.stringify(results.violations, null, 2)).toEqual([]);
  });
}

test("shared skip link reaches each product's primary content", async ({ page }) => {
  for (const path of ["", "home/", "creche/", "patchbay/"]) {
    await page.goto(`${entrance.url}${path}`);
    if (path === "creche/") {
      await expect(page.locator("#host-state")).toHaveText("Crèche ready");
      await expect(page.locator('[data-application-key="creche-heading"]')).toBeFocused();
    }
    if (path === "patchbay/") await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    if (path === "home/") await expect(page.locator("#host-state")).toHaveText("Browser Home is ready.");
    const skip = page.getByRole("link", { name: "Skip to main content" });
    await skip.focus();
    await expect(skip).toBeFocused();
    await skip.press("Enter");
    await expect(page.getByRole("main")).toBeFocused();
  }
});

test("the compatibility Crèche uses focused headings instead of broad live regions", async ({ page }) => {
  await page.goto(`${entrance.url}creche/`);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page.locator("#workspace")).not.toHaveAttribute("aria-live", /.+/);
  await openCrecheStep(page, "2. First Host");
  await expect(page.locator("#workspace h2")).toBeFocused();
});
