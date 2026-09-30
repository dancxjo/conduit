import { stageLegacyCrecheRoute } from "../../products/creche/tools/stage-legacy-routes.mjs";
import { cp, mkdir, rm } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

const pagesRoot = "target/pages-front-door-proof";
let entrance;

async function assemblePagesCarrier() {
  await rm(pagesRoot, { recursive: true, force: true });
  await mkdir(pagesRoot, { recursive: true });
  await cp("target/pages-root", pagesRoot, { recursive: true });
  await cp("target/creche-product", `${pagesRoot}/creche`, { recursive: true });
  await cp("target/workspace-product", `${pagesRoot}/workspace`, { recursive: true });
  await cp("target/patchbay-product", `${pagesRoot}/patchbay`, { recursive: true });
  await stageLegacyCrecheRoute(pagesRoot);
}

test.beforeAll(async () => {
  // Copying the complete accepted Pages carrier is bounded setup work, but its
  // several product trees can legitimately exceed Playwright's generic 20 s
  // hook timeout on a cold release runner. Keep browser actions on the normal
  // strict timeout while giving this one filesystem assembly an explicit cap.
  test.setTimeout(60_000);
  await assemblePagesCarrier();
});

test.beforeEach(async () => {
  entrance = await startStaticProduct(pagesRoot, "/conduit/");
});

test.afterEach(() => entrance?.child.kill());

test("Open your body enters the current Body surface", async ({ page }) => {
  await page.goto(entrance.url);
  await page.getByRole("link", { name: "Open your body", exact: true }).click();
  await expect(page).toHaveURL(/\/workspace\/$/);
  // Readiness is a Body fact, not the incidental layout of an empty main.
  await expect(page.locator("[data-body-state]")).not.toHaveText("Opening…");
  await expect(page.getByRole("main")).toBeVisible();
});

test("the main site exposes exact reviewed host and ConduitOS releases", async ({ page }) => {
  await page.goto(entrance.url);
  await page.getByRole("link", { name: "Get Conduit" }).click();
  await expect(page).toHaveURL(/#get-conduit$/);
  await expect(page.getByRole("heading", { name: "Run it here. Or boot the whole machine." })).toBeVisible();

  const expected = new Map([
    ["Linux x86_64 executable Download", "/conduit/creche/artifacts/conduit-linux-x86_64"],
    ["Windows x86_64 executable Download", "/conduit/creche/artifacts/conduit-windows-x86_64.exe"],
    ["macOS Apple silicon executable Download", "/conduit/creche/artifacts/conduit-macos-aarch64"],
    ["Browser WASM host page Open", "/conduit/creche/artifacts/index.html"],
    ["PC · x86_64 Q35 · UEFI ISO", "/conduit/creche/artifacts/conduitos-x86_64-pc.iso"],
    ["PC · IA-32 Legacy PC profile ISO", "/conduit/creche/artifacts/conduitos-ia32-pc.iso"],
    ["AArch64 QEMU virt · UEFI ISO", "/conduit/creche/artifacts/conduitos-aarch64-virt.iso"],
    ["RISC-V 64 QEMU virt · UEFI ISO", "/conduit/creche/artifacts/conduitos-riscv64-virt.iso"],
    ["LoongArch64 QEMU virt · UEFI ISO", "/conduit/creche/artifacts/conduitos-loongarch64-virt.iso"],
  ]);
  for (const [name, href] of expected) {
    await expect(page.getByRole("link", { name, exact: true })).toHaveAttribute("href", href);
  }
  await expect(page.getByLabel("Download and installation boundary")).toContainText("generic reviewed releases");
  await expect(page.getByLabel("Download and installation boundary")).toContainText("Downloading an image is not proof that it booted on your hardware");
});

test("current product truth exposes exact identities, lag, and proof classes", async ({ page }) => {
  const commit = (digit) => digit.repeat(40);
  await page.route("**/product-truth.json", (route) => route.fulfill({
    contentType: "application/json",
    body: JSON.stringify({
      schema: "conduit.product-truth/v1",
      repository: "dancxjo/conduit",
      development: { commit: commit("d"), integration_run_url: "https://example/dev" },
      accepted_release: {
        source_commit: commit("a"), main_commit: commit("b"), pull_request: 42,
      },
      publication: {
        release_main_commit: commit("b"),
        pages_url: "https://example/pages",
        deployment_url: "https://example/deployment",
      },
      evidence: [
        {
          surface: "browser products", proof_class: "live-browser",
          receipt_url: "https://example/browser", commit: commit("a"),
        },
        {
          surface: "ConduitOS visual journey", proof_class: "freestanding-emulator",
          receipt_url: "https://example/emulator", commit: commit("a"),
        },
      ],
      lag: {
        accepted_release_behind_development: true,
        publication_behind_accepted_release: false,
      },
    }),
  }));
  await page.goto(`${entrance.url}current-product.html`);
  await expect(page.getByRole("heading", { name: "Current product truth" })).toBeVisible();
  await expect(page.getByText("the accepted release is behind development", { exact: false })).toBeVisible();
  await expect(page.getByText("browser products — live-browser", { exact: false })).toBeVisible();
  await expect(page.getByText("ConduitOS visual journey — freestanding-emulator", { exact: false })).toBeVisible();
  await expect(page.locator("#product-truth")).toHaveAttribute("aria-busy", "false");
});

test("the shared shell follows dark and light preferences without changing application behavior", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  for (const colorScheme of ["dark", "light"]) {
    await page.emulateMedia({ colorScheme });
    for (const path of ["", "/patchbay/"]) {
      await page.goto(path === "" ? `${home}/` : `${home}${path}`);
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
      expect(palette.background).toBe(colorScheme === "dark" ? "#05070b" : "#eef5f8");
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
        ? page.getByRole("link", { name: "Open your body" })
        : primaryNavigation.getByRole("link", { name: "Patchbay" });
      await page.keyboard.press("Tab");
      await focusTarget.focus();
      await expect(focusTarget).toHaveCSS(
        "outline-color",
        colorScheme === "dark" ? "rgb(244, 196, 0)" : "rgb(119, 93, 0)",
      );
    }
  }
});
