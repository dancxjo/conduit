import { stageLegacyTourRoutes } from "../../products/tour/tools/stage-legacy-routes.mjs";
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
  await cp("target/tour-product", `${pagesRoot}/tour`, { recursive: true });
  await cp("target/creche-product", `${pagesRoot}/creche`, { recursive: true });
  await cp("target/workspace-product", `${pagesRoot}/workspace`, { recursive: true });
  await cp("target/patchbay-product", `${pagesRoot}/patchbay`, { recursive: true });
  await cp("target/home-product", `${pagesRoot}/home`, { recursive: true });
  await stageLegacyTourRoutes(pagesRoot);
  await stageLegacyCrecheRoute(pagesRoot);
}

test.beforeAll(async () => {
  await assemblePagesCarrier();
});

test.beforeEach(async () => {
  entrance = await startStaticProduct(pagesRoot, "/conduit/");
});

test.afterEach(() => entrance?.child.kill());

test("Birth on the front page hands the Lulled Body to an explicit Wake", async ({ page }) => {
  await page.goto(entrance.url);
  await page.getByRole("link", { name: "Open your Body", exact: true }).click();
  await expect(page).toHaveTitle("Birth your Body · Conduit");
  await expect(page.locator("[data-body-state]")).toHaveText("Crèche");
  await expect(page.locator("[data-workspace-creche]")).toBeVisible();
  await expect(page.getByRole("button", { name: "Birth Body", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-body-state]")).toHaveText("lulled");
  await expect(page.getByRole("button", { name: "Wake Body", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Wake Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await page.keyboard.press("h");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("h");
});

test("Browser Home enacts the shared journey through the real Patchbay", async ({ page }) => {
  const root = entrance.url.replace(/\/$/, "");
  const steps = ["home.arrived"];
  await page.goto(`${root}/home/`);
  await expect(page.locator("#host-state")).toHaveText("Browser Home is ready.");
  await page.getByRole("button", { name: "FORMS" }).click();
  steps.push("forms.opened");
  await page.getByRole("button", { name: "Hello" }).click();
  steps.push("form.selected");
  const command = page.getByLabel("conduit>");
  await command.fill("open prompt");
  await command.press("Enter");
  steps.push("prompt.opened");
  await command.fill("run hello");
  await command.press("Enter");
  await expect(page.locator("#host-state")).toHaveAttribute("data-play-disposition", "completed");
  steps.push("form.run", "play.observed");
  await command.fill("home");
  await command.press("Enter");
  await page.getByRole("button", { name: "PATCHBAY" }).click();
  await expect(page).toHaveURL(`${root}/patchbay/`);
  await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  steps.push("patchbay.opened");
  await page.goBack();
  await expect(page.getByRole("button", { name: "TOUR" })).toBeVisible();
  steps.push("home.returned");
  expect(steps).toEqual([
    "home.arrived", "forms.opened", "form.selected", "prompt.opened",
    "form.run", "play.observed", "patchbay.opened", "home.returned",
  ]);
});

test.skip("Conduit home makes the Body primary while remaining compatibility endpoints stay reachable", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  const tour = `${home}/tour/`;
  const creche = `${home}/creche/`;
  const patchbay = `${home}/patchbay/`;
  const homeFace = `${home}/home/`;

  await page.goto(homeFace);
  await expect(page).toHaveURL(homeFace);
  await expect(page.locator("#host-state")).toHaveText("Browser Home is ready.");
  await expect(page.getByRole("button", { name: "FORMS" })).toBeVisible();

  await page.goto(`${home}/`);
  await expect(page).toHaveURL(`${home}/`);
  await expect(page.getByRole("heading", { name: "One Program, Many Computers" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Conduit home" })).toHaveAttribute("href", "/conduit");
  await expect(page.getByRole("link", { name: "Follow a real journey" })).toHaveAttribute("href", "/conduit/journeys/");
  await expect(page.getByRole("link", { name: "Open your Body", exact: true })).toHaveAttribute("href", "/conduit/workspace/");
  await expect(page.getByRole("heading", { name: "Start with the Body." })).toBeVisible();
  await expect(page.getByText("With no retained Body, this entrance presents bounded setup.")).toBeVisible();
  await expect(page.getByRole("link", { name: /Your Body/ })).toHaveAttribute("href", "/conduit/workspace/");
  await expect(page.getByText("The tutorial and zero-Body setup now belong to this Body-centered surface.")).toBeVisible();
  await expect(page.getByText("zero-Body setup now belong to this Body-centered surface", { exact: false })).toBeVisible();
  await expect(page.getByRole("link", { name: "standalone Patchbay", exact: true })).toHaveAttribute("href", "/conduit/patchbay/");
  await expect(page.getByRole("link", { name: "Get Conduit" })).toHaveAttribute("href", "#get-conduit");
  await expect(page.getByText("One physical computer")).toBeVisible();
  await expect(page.getByText("Several unlike computers")).toBeVisible();
  const productNavigation = page.getByRole("navigation", { name: "Conduit products" });
  const productLinks = productNavigation.getByRole("link");
  await expect(productLinks).toHaveCount(3);
  expect(await productLinks.allTextContents()).toEqual(["conduit", "Patchbay", "Source"]);
  expect(await productLinks.evaluateAll((links) => links.map((link) => ({ tag: link.tagName, target: link.target, onclick: link.onclick })))).toEqual([
    { tag: "A", target: "", onclick: null },
    { tag: "A", target: "", onclick: null },
    { tag: "A", target: "", onclick: null },
  ]);
  await page.getByRole("link", { name: "Patchbay", exact: true }).first().click();
  await expect(page).toHaveURL(patchbay);
  await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  await page.goBack();
  await expect(page).toHaveURL(`${home}/`);
  await page.goForward();
  await expect(page).toHaveURL(patchbay);
  await expect(page.locator("body")).toHaveAttribute("data-embodied", "false");

  await page.goto(tour);
  await expect(page).toHaveURL(`${home}/workspace/`);
  await expect(page.locator("[data-body-tutorial]")).toBeVisible();

  await page.goto(`${creche}?from=legacy#birth`);
  await expect(page).toHaveURL(`${home}/workspace/?from=legacy#birth`);
  await expect(page.locator("[data-workspace-creche]")).toBeVisible();
  await page.goto(patchbay);

  await expect(page).toHaveTitle("Conduit Patchbay");
  await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  await expect(page.locator("body")).toHaveAttribute("data-embodied", "false");
  await expect(page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Patchbay" })).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Tour" })).toHaveCount(0);
  await expect(page.getByRole("navigation", { name: "Conduit products" }).getByRole("link", { name: "Crèche" })).toHaveCount(0);
  await page.reload();
  await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  await expect(page.locator("body")).toHaveAttribute("data-embodied", "false");
});

test("the main site exposes exact reviewed Host and ConduitOS releases", async ({ page }) => {
  await page.goto(entrance.url);
  await page.getByRole("link", { name: "Get Conduit" }).click();
  await expect(page).toHaveURL(/#get-conduit$/);
  await expect(page.getByRole("heading", { name: "Run it here. Or boot the whole machine." })).toBeVisible();

  const expected = new Map([
    ["Linux x86_64 executable Download", "/conduit/creche/artifacts/conduit-linux-x86_64"],
    ["Windows x86_64 executable Download", "/conduit/creche/artifacts/conduit-windows-x86_64.exe"],
    ["macOS Apple silicon executable Download", "/conduit/creche/artifacts/conduit-macos-aarch64"],
    ["Browser WASM Host page Open", "/conduit/creche/artifacts/index.html"],
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

test("published Tour chapter permalinks converge on the tutorial-enabled Body", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  const permalinks = [
    "meet-one-gear",
    "same-face-different-implementation",
  ];

  for (const slug of permalinks) {
    await page.goto(`${home}/tour/${slug}/?from=legacy#learn`);
    await expect(page).toHaveURL(`${home}/workspace/?from=legacy#learn`);
    await expect(page.locator("[data-body-tutorial]")).toBeVisible();
  }
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

test("legacy /book routes redirect to the tutorial-enabled Body", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  await page.goto(`${home}/book/`);
  await expect(page).toHaveURL(`${home}/workspace/`);
  await expect(page.locator("[data-body-tutorial]")).toBeVisible();
  await page.goto(`${home}/book/meet-one-gear/?from=legacy#source`);
  await expect(page).toHaveURL(`${home}/workspace/?from=legacy#source`);
  await expect(page.locator("[data-body-tutorial]")).toBeVisible();
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
        ? page.getByRole("link", { name: "Open your Body" })
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
