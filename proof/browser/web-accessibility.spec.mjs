import AxeBuilder from "@axe-core/playwright";
import { cp, mkdir, rm } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { stageLegacyCrecheRoute } from "../../products/creche/tools/stage-legacy-routes.mjs";
import { startStaticProduct } from "./static-product-server.mjs";

const pagesRoot = "target/web-accessibility-proof";
let entrance;

test.beforeAll(async () => {
  await rm(pagesRoot, { recursive: true, force: true });
  await mkdir(pagesRoot, { recursive: true });
  await cp("target/pages-root", pagesRoot, { recursive: true });
  await cp("target/creche-product", `${pagesRoot}/creche`, { recursive: true });
  await cp("target/workspace-product", `${pagesRoot}/workspace`, { recursive: true });
  await cp("target/patchbay-product", `${pagesRoot}/patchbay`, { recursive: true });
  await stageLegacyCrecheRoute(pagesRoot);
  entrance = await startStaticProduct(pagesRoot, "/conduit/");
});

test.afterAll(() => entrance?.child.kill());

for (const [name, path] of [["Home", ""], ["Body", "workspace/"], ["Patchbay", "patchbay/"]]) {
  test(`${name} rendered entrance has WCAG 2.2 AA structure`, async ({ page }) => {
    await page.goto(`${entrance.url}${path}`);
    if (name === "Body") await expect(page.locator("[data-body-tutorial]")).toBeVisible();
    if (name === "Patchbay") await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    await expect(page.locator("#conduit-suspense")).toHaveCount(0);
    await expect(page.getByRole("main")).toHaveCount(1);
    expect(await page.getByRole("heading", { level: 1 }).count()).toBeGreaterThan(0);
    const results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]).analyze();
    expect(results.violations, JSON.stringify(results.violations, null, 2)).toEqual([]);
  });
}

test("shared skip link reaches each product's primary content", async ({ page }) => {
  for (const path of ["", "patchbay/"]) {
    await page.goto(`${entrance.url}${path}`);
    if (path === "patchbay/") await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    const skip = page.getByRole("link", { name: "Skip to main content" });
    await skip.focus();
    await expect(skip).toBeFocused();
    await skip.press("Enter");
    await expect(page.getByRole("main")).toBeFocused();
  }
});
