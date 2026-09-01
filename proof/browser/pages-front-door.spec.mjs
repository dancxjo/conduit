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

  await page.goto(book);
  await expect(page).toHaveURL(book);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "Meet one Gear" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Conduit home" })).toHaveAttribute("href", "/conduit");

  await page.goto(creche);
  await expect(page).toHaveURL(creche);
  await expect(page).toHaveTitle("Conduit Crèche");
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page.getByRole("link", { name: "Conduit home" })).toHaveAttribute("href", "/conduit");
});
