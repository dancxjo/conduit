# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: pages-front-door.spec.mjs >> Open your body enters the current Body surface
- Location: proof/browser/pages-front-door.spec.mjs:35:1

# Error details

```
Error: expect(locator).not.toHaveText(expected) failed

Locator:  locator('[data-body-state]')
Expected: not "Opening…"
Received: "Opening…"
Timeout:  15000ms

Call log:
  - Expect "not toHaveText" with timeout 15000ms
  - waiting for locator('[data-body-state]')
    33 × locator resolved to <span class="body-state" data-body-state="">Opening…</span>
       - unexpected value "Opening…"

```

```yaml
- text: Opening…
```

# Test source

```ts
  1   | import { stageLegacyCrecheRoute } from "../../products/creche/tools/stage-legacy-routes.mjs";
  2   | import { cp, mkdir, rm } from "node:fs/promises";
  3   | import { expect, test } from "@playwright/test";
  4   | import { startStaticProduct } from "./tour-test-server.mjs";
  5   | 
  6   | const pagesRoot = "target/pages-front-door-proof";
  7   | let entrance;
  8   | 
  9   | async function assemblePagesCarrier() {
  10  |   await rm(pagesRoot, { recursive: true, force: true });
  11  |   await mkdir(pagesRoot, { recursive: true });
  12  |   await cp("target/pages-root", pagesRoot, { recursive: true });
  13  |   await cp("target/creche-product", `${pagesRoot}/creche`, { recursive: true });
  14  |   await cp("target/workspace-product", `${pagesRoot}/workspace`, { recursive: true });
  15  |   await cp("target/patchbay-product", `${pagesRoot}/patchbay`, { recursive: true });
  16  |   await cp("target/home-product", `${pagesRoot}/home`, { recursive: true });
  17  |   await stageLegacyCrecheRoute(pagesRoot);
  18  | }
  19  | 
  20  | test.beforeAll(async () => {
  21  |   // Copying the complete accepted Pages carrier is bounded setup work, but its
  22  |   // several product trees can legitimately exceed Playwright's generic 20 s
  23  |   // hook timeout on a cold release runner. Keep browser actions on the normal
  24  |   // strict timeout while giving this one filesystem assembly an explicit cap.
  25  |   test.setTimeout(60_000);
  26  |   await assemblePagesCarrier();
  27  | });
  28  | 
  29  | test.beforeEach(async () => {
  30  |   entrance = await startStaticProduct(pagesRoot, "/conduit/");
  31  | });
  32  | 
  33  | test.afterEach(() => entrance?.child.kill());
  34  | 
  35  | test("Open your body enters the current Body surface", async ({ page }) => {
  36  |   await page.goto(entrance.url);
  37  |   await page.getByRole("link", { name: "Open your body", exact: true }).click();
  38  |   await expect(page).toHaveURL(/\/workspace\/$/);
  39  |   // The compiled browser Host may take longer than a generic layout assertion
  40  |   // to instantiate on a cold runner. Wait for its semantic readiness fact;
  41  |   // only then does the Workspace promise a visible Body surface.
> 42  |   await expect(page.locator("[data-body-state]")).not.toHaveText("Opening…", { timeout: 15_000 });
      |                                                       ^ Error: expect(locator).not.toHaveText(expected) failed
  43  |   await expect(page.getByRole("main")).toBeVisible();
  44  | });
  45  | 
  46  | test("Browser Home enacts the shared journey through the real Patchbay", async ({ page }) => {
  47  |   const root = entrance.url.replace(/\/$/, "");
  48  |   const steps = ["home.arrived"];
  49  |   await page.goto(`${root}/home/`);
  50  |   await expect(page.locator("#host-state")).toHaveText("Browser Home is ready.");
  51  |   await page.getByRole("button", { name: "FORMS" }).click();
  52  |   steps.push("forms.opened");
  53  |   await page.getByRole("button", { name: "Hello" }).click();
  54  |   steps.push("form.selected");
  55  |   const command = page.getByLabel("conduit>");
  56  |   await command.fill("open prompt");
  57  |   await command.press("Enter");
  58  |   steps.push("prompt.opened");
  59  |   await command.fill("run hello");
  60  |   await command.press("Enter");
  61  |   await expect(page.locator("#host-state")).toHaveAttribute("data-play-disposition", "completed");
  62  |   steps.push("form.run", "play.observed");
  63  |   await command.fill("home");
  64  |   await command.press("Enter");
  65  |   await page.getByRole("button", { name: "PATCHBAY" }).click();
  66  |   await expect(page).toHaveURL(`${root}/patchbay/`);
  67  |   await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  68  |   steps.push("patchbay.requested");
  69  |   await page.goBack();
  70  |   await expect(page.getByRole("button", { name: "TOUR" })).toBeVisible();
  71  |   steps.push("home.returned");
  72  |   expect(steps).toEqual([
  73  |     "home.arrived", "forms.opened", "form.selected", "prompt.opened",
  74  |     "form.run", "play.observed", "patchbay.requested", "home.returned",
  75  |   ]);
  76  | });
  77  | 
  78  | test("the main site exposes exact reviewed host and ConduitOS releases", async ({ page }) => {
  79  |   await page.goto(entrance.url);
  80  |   await page.getByRole("link", { name: "Get Conduit" }).click();
  81  |   await expect(page).toHaveURL(/#get-conduit$/);
  82  |   await expect(page.getByRole("heading", { name: "Run it here. Or boot the whole machine." })).toBeVisible();
  83  | 
  84  |   const expected = new Map([
  85  |     ["Linux x86_64 executable Download", "/conduit/creche/artifacts/conduit-linux-x86_64"],
  86  |     ["Windows x86_64 executable Download", "/conduit/creche/artifacts/conduit-windows-x86_64.exe"],
  87  |     ["macOS Apple silicon executable Download", "/conduit/creche/artifacts/conduit-macos-aarch64"],
  88  |     ["Browser WASM host page Open", "/conduit/creche/artifacts/index.html"],
  89  |     ["PC · x86_64 Q35 · UEFI ISO", "/conduit/creche/artifacts/conduitos-x86_64-pc.iso"],
  90  |     ["PC · IA-32 Legacy PC profile ISO", "/conduit/creche/artifacts/conduitos-ia32-pc.iso"],
  91  |     ["AArch64 QEMU virt · UEFI ISO", "/conduit/creche/artifacts/conduitos-aarch64-virt.iso"],
  92  |     ["RISC-V 64 QEMU virt · UEFI ISO", "/conduit/creche/artifacts/conduitos-riscv64-virt.iso"],
  93  |     ["LoongArch64 QEMU virt · UEFI ISO", "/conduit/creche/artifacts/conduitos-loongarch64-virt.iso"],
  94  |   ]);
  95  |   for (const [name, href] of expected) {
  96  |     await expect(page.getByRole("link", { name, exact: true })).toHaveAttribute("href", href);
  97  |   }
  98  |   await expect(page.getByLabel("Download and installation boundary")).toContainText("generic reviewed releases");
  99  |   await expect(page.getByLabel("Download and installation boundary")).toContainText("Downloading an image is not proof that it booted on your hardware");
  100 | });
  101 | 
  102 | test("current product truth exposes exact identities, lag, and proof classes", async ({ page }) => {
  103 |   const commit = (digit) => digit.repeat(40);
  104 |   await page.route("**/product-truth.json", (route) => route.fulfill({
  105 |     contentType: "application/json",
  106 |     body: JSON.stringify({
  107 |       schema: "conduit.product-truth/v1",
  108 |       repository: "dancxjo/conduit",
  109 |       development: { commit: commit("d"), integration_run_url: "https://example/dev" },
  110 |       accepted_release: {
  111 |         source_commit: commit("a"), main_commit: commit("b"), pull_request: 42,
  112 |       },
  113 |       publication: {
  114 |         release_main_commit: commit("b"),
  115 |         pages_url: "https://example/pages",
  116 |         deployment_url: "https://example/deployment",
  117 |       },
  118 |       evidence: [
  119 |         {
  120 |           surface: "browser products", proof_class: "live-browser",
  121 |           receipt_url: "https://example/browser", commit: commit("a"),
  122 |         },
  123 |         {
  124 |           surface: "ConduitOS visual journey", proof_class: "freestanding-emulator",
  125 |           receipt_url: "https://example/emulator", commit: commit("a"),
  126 |         },
  127 |       ],
  128 |       lag: {
  129 |         accepted_release_behind_development: true,
  130 |         publication_behind_accepted_release: false,
  131 |       },
  132 |     }),
  133 |   }));
  134 |   await page.goto(`${entrance.url}current-product.html`);
  135 |   await expect(page.getByRole("heading", { name: "Current product truth" })).toBeVisible();
  136 |   await expect(page.getByText("the accepted release is behind development", { exact: false })).toBeVisible();
  137 |   await expect(page.getByText("browser products — live-browser", { exact: false })).toBeVisible();
  138 |   await expect(page.getByText("ConduitOS visual journey — freestanding-emulator", { exact: false })).toBeVisible();
  139 |   await expect(page.locator("#product-truth")).toHaveAttribute("aria-busy", "false");
  140 | });
  141 | 
  142 | test("the shared shell follows dark and light preferences without changing application behavior", async ({ page }) => {
```