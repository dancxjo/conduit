import { spawn } from "node:child_process";
import { cp, mkdir, rm } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./book-test-server.mjs";

const pagesRoot = "target/patchbay-product-proof";
const evidencePath = `${pagesRoot}/body-evidence.json`;
let entrance;
let admission;

async function stageCarrier() {
  await rm(pagesRoot, { recursive: true, force: true });
  await mkdir(pagesRoot, { recursive: true });
  await cp("target/pages-root", pagesRoot, { recursive: true });
  await cp("target/book-product", `${pagesRoot}/book`, { recursive: true });
  await cp("target/creche-product", `${pagesRoot}/creche`, { recursive: true });
  await cp("target/patchbay-product", `${pagesRoot}/patchbay`, { recursive: true });
}

function startAdmissionProbe() {
  admission = spawn("target/debug/browser-admission-probe", ["--presence", "--biography", evidencePath], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`admission probe was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/(ws:\/\/127\.0\.0\.1:\d+)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    admission.stdout.on("data", inspect);
    admission.stderr.on("data", inspect);
    admission.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`admission probe exited (${code})\n${output}`));
    });
  });
}

test.beforeAll(async () => {
  await stageCarrier();
  entrance = await startStaticProduct(pagesRoot, "/conduit/");
});

test.afterAll(() => {
  admission?.kill();
  entrance?.child.kill();
});

test("canonical Patchbay route starts as an honest bounded no-Body workbench", async ({ page }) => {
  const patchbay = `${entrance.url.replace(/\/$/, "")}/patchbay/`;
  await page.goto(patchbay);
  await expect(page).toHaveURL(patchbay);
  await expect(page).toHaveTitle("Conduit Patchbay");
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("navigation", { name: "Primary" }).getByRole("link", { name: "Patchbay" })).toHaveAttribute("aria-current", "page");
  await expect(page.getByText("No Body open · Patchbay remains available")).toBeVisible();
  await expect(page.getByText("No Body evidence is open. Patchbay has not created, joined, or inferred one.")).toBeVisible();
  expect(await page.evaluate(() => globalThis.__conduitPatchbay.relationship())).toBe("none");
  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  expect(await page.evaluate(() => globalThis.__conduitPatchbay.relationship())).toBe("none");
});

test("one Patchbay route distinguishes hosted membership from external reading", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  const websocket = await startAdmissionProbe();
  const parameters = new URLSearchParams({
    mode: "hosted",
    body: websocket,
    evidence: `${home}/body-evidence.json`,
    plan: "plan/patchbay-pages-proof",
    implementation: "browser/patchbay-surface@1",
  });
  await page.goto(`${home}/patchbay/?${parameters}`);
  await expect.poll(() => page.evaluate(() => globalThis.__conduitPatchbay?.relationship() ?? "starting")).not.toBe("starting");
  await page.waitForTimeout(750);
  const hostedDiagnostic = await page.evaluate(() => ({
    relationship: globalThis.__conduitPatchbay.relationship(),
    text: document.body.innerText,
  }));
  expect(hostedDiagnostic.relationship, hostedDiagnostic.text).toBe("hosted");
  await expect(page.getByText("Hosted Patchbay · this browser Host is a current member")).toBeVisible();
  await expect(page.getByLabel("Relationship to a Body").getByText("Current Host/Boot membership and the exact Patchbay placement are validated.")).toBeVisible();
  const hosted = await page.evaluate(() => ({
    body: globalThis.__conduitPatchbay.membership().bodyId(),
    host: globalThis.__conduitPatchbay.host.hostId,
    boot: globalThis.__conduitPatchbay.host.bootId,
  }));
  expect(hosted.body).toMatch(/^[0-9a-f]{64}$/);
  await expect(page.getByText(hosted.host, { exact: true })).toBeVisible();
  await expect(page.getByText(hosted.boot, { exact: true })).toBeVisible();

  await page.goto(`${home}/patchbay/?${new URLSearchParams({ mode: "external", evidence: `${home}/body-evidence.json` })}`);
  await expect.poll(() => page.evaluate(() => globalThis.__conduitPatchbay?.relationship() ?? "starting")).toBe("external");
  await expect(page.getByText("External Patchbay · this browser Host is not part of the viewed Body")).toBeVisible();
  await expect(page.getByText("not a member", { exact: true })).toHaveCount(2);
  expect(await page.evaluate(() => globalThis.__conduitPatchbay.membership())).toBeNull();
});

test("Patchbay peer links preserve ordinary navigation, back, and direct deep links", async ({ page }) => {
  const home = entrance.url.replace(/\/$/, "");
  await page.goto(`${home}/patchbay/`);
  await page.getByRole("navigation", { name: "Primary" }).getByRole("link", { name: "Book" }).click();
  await expect(page).toHaveURL(`${home}/book/`);
  await page.goBack();
  await expect(page).toHaveURL(`${home}/patchbay/`);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await page.goto(`${home}/patchbay/index.html`);
  await expect(page.getByRole("heading", { name: "Patchbay", exact: true })).toBeVisible();
});
