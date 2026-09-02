import { cp, mkdir, rm } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./book-test-server.mjs";

const pagesRoot = "target/pages-front-door-proof";
let entrance;

async function assemblePagesCarrier() {
  await rm(pagesRoot, { recursive: true, force: true });
  await mkdir(pagesRoot, { recursive: true });
  await cp("target/pages-root", pagesRoot, { recursive: true });
  await cp("target/book-product", `${pagesRoot}/book`, { recursive: true });
  await cp("target/creche-product", `${pagesRoot}/creche`, { recursive: true });
}

test.beforeAll(async () => {
  await assemblePagesCarrier();
});

test.beforeEach(async () => {
  entrance = await startStaticProduct(pagesRoot, "/conduit/");
});

test.afterEach(() => entrance?.child.kill());

test("Conduit home, Book, and Crèche are stable sibling endpoints", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  const book = `${home}/book`;
  const creche = `${home}/creche`;

  await page.goto(home);
  await expect(page).toHaveURL(home);
  await expect(page.getByRole("heading", { name: /One logical computer/i })).toBeVisible();
  await expect(page.getByRole("link", { name: "Conduit home" })).toHaveAttribute("href", "/conduit");
  await expect(page.getByRole("link", { name: "Learn Conduit" })).toHaveAttribute("href", "/conduit/book");
  await expect(page.getByRole("link", { name: "Birth a Body" })).toHaveAttribute("href", "/conduit/creche");
  await expect(page.getByText("One physical computer")).toBeVisible();
  await expect(page.getByText("Several unlike computers")).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Primary" }).getByRole("link")).toHaveCount(3);

  await page.goto(book);
  await expect(page).toHaveURL(book);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "A Form you can run" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Conduit home" })).toHaveAttribute("href", "/conduit");
  await expect(page.getByRole("navigation", { name: "Primary" }).getByRole("link", { name: "Book" })).toHaveAttribute("aria-current", "page");

  await page.goto(creche);
  await expect(page).toHaveURL(creche);
  await expect(page).toHaveTitle("Conduit Crèche");
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page.getByRole("link", { name: "Conduit home" })).toHaveAttribute("href", "/conduit");
  await expect(page.getByRole("navigation", { name: "Primary" }).getByRole("link", { name: "Crèche" })).toHaveAttribute("aria-current", "page");
});

test("the shared shell follows dark and light preferences without changing application behavior", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  for (const colorScheme of ["dark", "light"]) {
    await page.emulateMedia({ colorScheme });
    for (const path of ["", "/book", "/creche"]) {
      await page.goto(`${home}${path}`);
      if (path === "/book") await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
      if (path === "/creche") await expect(page.locator("#host-state")).toHaveText("Crèche ready");
      const palette = await page.evaluate(() => ({
        scheme: getComputedStyle(document.documentElement).colorScheme,
        background: getComputedStyle(document.documentElement).getPropertyValue(
          location.pathname.endsWith("/conduit/") ? "--conduit-background" : "--paper",
        ).trim(),
        accent: getComputedStyle(document.querySelector(
          location.pathname.endsWith("/conduit/") ? ".primary-action" : "h1",
        ))[location.pathname.endsWith("/conduit/") ? "backgroundColor" : "color"],
      }));
      expect(palette.scheme).toContain(colorScheme);
      expect(palette.background).toBe(colorScheme === "dark" ? "#05070b" : "#eef5f8");
      expect(palette.accent).toBe(colorScheme === "dark" ? "rgb(233, 163, 37)" : "rgb(154, 91, 0)");
      const primaryNavigation = page.getByRole("navigation", { name: "Primary" });
      await expect(primaryNavigation).toBeVisible();
      const hoverTarget = primaryNavigation.getByRole("link", { name: "Source" });
      await hoverTarget.hover();
      await expect(hoverTarget).toHaveCSS(
        "color",
        path === ""
          ? colorScheme === "dark" ? "rgb(233, 163, 37)" : "rgb(154, 91, 0)"
          : colorScheme === "dark" ? "rgb(147, 210, 247)" : "rgb(23, 54, 77)",
      );
      const focusTarget = path === ""
        ? page.getByRole("link", { name: "Learn Conduit" })
        : primaryNavigation.getByRole("link", { name: path === "/book" ? "Book" : "Crèche" });
      await page.keyboard.press("Tab");
      await focusTarget.focus();
      await expect(focusTarget).toHaveCSS(
        "outline-color",
        colorScheme === "dark" ? "rgb(244, 196, 0)" : "rgb(119, 93, 0)",
      );
    }
  }
});
