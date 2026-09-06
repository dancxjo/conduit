import { stageLegacyTourRoutes } from "../../products/tour/tools/stage-legacy-routes.mjs";
import { cp, mkdir, rm } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

const pagesRoot = "target/pages-front-door-proof";
let entrance;

async function assemblePagesCarrier() {
  await rm(pagesRoot, { recursive: true, force: true });
  await mkdir(pagesRoot, { recursive: true });
  await cp("target/pages-root", pagesRoot, { recursive: true });
  await cp("target/tour-product", `${pagesRoot}/tour`, { recursive: true });
  await cp("target/creche-product", `${pagesRoot}/creche`, { recursive: true });
  await cp("target/patchbay-product", `${pagesRoot}/patchbay`, { recursive: true });
  await stageLegacyTourRoutes(pagesRoot);
}

test.beforeAll(async () => {
  await assemblePagesCarrier();
});

test.beforeEach(async () => {
  entrance = await startStaticProduct(pagesRoot, "/conduit/");
});

test.afterEach(() => entrance?.child.kill());

test("Conduit home, Tour, Crèche, and Patchbay are stable sibling endpoints", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  const tour = `${home}/tour/`;
  const creche = `${home}/creche/`;
  const patchbay = `${home}/patchbay/`;

  await page.goto(`${home}/`);
  await expect(page).toHaveURL(`${home}/`);
  await expect(page.getByRole("heading", { name: /One logical computer/i })).toBeVisible();
  await expect(page.getByRole("link", { name: "Conduit home" })).toHaveAttribute("href", "/conduit");
  await expect(page.getByRole("link", { name: "Learn Conduit" })).toHaveAttribute("href", "/conduit/tour");
  await expect(page.getByRole("link", { name: "Birth a Body", exact: true })).toHaveAttribute("href", "/conduit/creche");
  await expect(page.getByText("One physical computer")).toBeVisible();
  await expect(page.getByText("Several unlike computers")).toBeVisible();
  await expect(page.getByRole("link", { name: "Patchbay", exact: true }).first()).toHaveAttribute("href", "/conduit/patchbay/");
  const productNavigation = page.getByRole("navigation", { name: "Conduit products" });
  const productLinks = productNavigation.getByRole("link");
  await expect(productLinks).toHaveCount(5);
  expect(await productLinks.allTextContents()).toEqual(["conduit", "Tour", "Crèche", "Patchbay", "Source"]);
  expect(await productLinks.evaluateAll((links) => links.map((link) => ({ tag: link.tagName, target: link.target, onclick: link.onclick })))).toEqual([
    { tag: "A", target: "", onclick: null },
    { tag: "A", target: "", onclick: null },
    { tag: "A", target: "", onclick: null },
    { tag: "A", target: "", onclick: null },
    { tag: "A", target: "", onclick: null },
  ]);
  const [newTab] = await Promise.all([
    page.context().waitForEvent("page"),
    productNavigation.getByRole("link", { name: "Tour" }).click({ button: "middle" }),
  ]);
  try {
    await expect(newTab).toHaveURL(`${tour}a-form-you-can-run/`);
    await expect(newTab.getByRole("heading", { name: "A Form you can run" })).toBeVisible();
  } finally { await newTab.close(); }
  await productNavigation.getByRole("link", { name: "Tour" }).focus();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(tour);
  await page.goBack();
  await expect(page).toHaveURL(`${home}/`);

  await page.getByRole("link", { name: "Patchbay", exact: true }).first().click();
  await expect(page).toHaveURL(patchbay);
  await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  await page.goBack();
  await expect(page).toHaveURL(`${home}/`);
  await page.goForward();
  await expect(page).toHaveURL(patchbay);
  await expect(page.locator("body")).toHaveAttribute("data-embodied", "false");

  await page.goto(tour);
  await expect(page).toHaveURL(tour);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "A Form you can run" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Conduit home" })).toHaveAttribute("href", "/conduit");
  await expect(page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Tour" })).toHaveAttribute("aria-current", "page");
  await page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Patchbay" }).click();
  await expect(page).toHaveURL(patchbay);

  await page.goto(creche);
  await expect(page).toHaveURL(creche);
  await expect(page).toHaveTitle("Conduit Crèche");
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page.getByRole("link", { name: "Conduit home" })).toHaveAttribute("href", "/conduit");
  await expect(page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Crèche" })).toHaveAttribute("aria-current", "page");
  await page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Patchbay" }).click();
  await expect(page).toHaveURL(patchbay);

  await expect(page).toHaveTitle("Conduit Patchbay");
  await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  await expect(page.locator("body")).toHaveAttribute("data-embodied", "false");
  await expect(page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Patchbay" })).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Tour" })).toHaveAttribute("href", "/conduit/tour/");
  await expect(page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Crèche" })).toHaveAttribute("href", "/conduit/creche/");
  await page.reload();
  await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  await expect(page.locator("body")).toHaveAttribute("data-embodied", "false");
});

test("published Tour chapter permalinks remain deployable", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  const permalinks = [
    ["meet-one-gear", "A Form you can run"],
    ["same-face-different-implementation", "Faces, Backs, and implementation"],
  ];

  for (const [slug, title] of permalinks) {
    await page.goto(`${home}/tour/${slug}/`);
    await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
    await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
  }
});

test("legacy /book routes redirect to canonical /tour routes", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  await page.goto(`${home}/book/`);
  await expect(page).toHaveURL(`${home}/tour/`);
  await expect(page.getByRole("heading", { name: "A Form you can run" })).toBeVisible();
  await page.goto(`${home}/book/meet-one-gear/?from=legacy#source`);
  await expect(page).toHaveURL(`${home}/tour/meet-one-gear/?from=legacy#source`);
  await expect(page.getByRole("heading", { level: 1, name: "A Form you can run" })).toBeVisible();
});

test("the shared shell follows dark and light preferences without changing application behavior", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  for (const colorScheme of ["dark", "light"]) {
    await page.emulateMedia({ colorScheme });
    for (const path of ["", "/tour/", "/creche/", "/patchbay/"]) {
      await page.goto(path === "" ? `${home}/` : `${home}${path}`);
      if (path === "/tour/") await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
      if (path === "/creche/") await expect(page.locator("#host-state")).toHaveText("Crèche ready");
      if (path === "/patchbay/") await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
      const palette = await page.evaluate(() => {
        const patchbay = location.pathname.endsWith("/patchbay/");
        const style = getComputedStyle(patchbay ? document.body : document.documentElement);
        return {
          scheme: getComputedStyle(document.documentElement).colorScheme,
          background: style.getPropertyValue(
            location.pathname.endsWith("/conduit/") || patchbay ? "--conduit-background" : "--paper",
          ).trim().toLowerCase(),
        };
      });
      expect(palette.scheme).toContain(colorScheme);
      expect(palette.background).toBe(colorScheme === "dark"
        ? "#05070b"
        : path === "/patchbay/" ? "#e5eff4" : "#eef5f8");
      const primaryNavigation = page.getByRole("navigation", { name: "Conduit products" });
      await expect(primaryNavigation).toBeVisible();
      const hoverTarget = primaryNavigation.getByRole("link", {
        name: path === "/patchbay/" ? "conduit" : "Patchbay",
      });
      await hoverTarget.hover();
      await expect(hoverTarget).toHaveCSS(
        "color",
        colorScheme === "dark" ? "rgb(147, 210, 247)" : "rgb(23, 54, 77)",
      );
      const focusTarget = path === ""
        ? page.getByRole("link", { name: "Learn Conduit" })
        : primaryNavigation.getByRole("link", { name: path === "/tour/" ? "Tour" : path === "/creche/" ? "Crèche" : "Patchbay" });
      await page.keyboard.press("Tab");
      await focusTarget.focus();
      await expect(focusTarget).toHaveCSS(
        "outline-color",
        colorScheme === "dark" ? "rgb(244, 196, 0)" : "rgb(119, 93, 0)",
      );
    }
  }
});
