import { createHash } from "node:crypto";
import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

const root = "target/home-cross-front-browser-proof";
const sharedSteps = [
  "home.arrived", "forms.opened", "form.selected", "prompt.opened",
  "form.run", "play.observed", "patchbay.opened", "home.returned",
];
let entrance;

test.beforeAll(async () => {
  await rm(root, { recursive: true, force: true });
  await mkdir(root, { recursive: true });
  await cp("target/pages-root", root, { recursive: true });
  await cp("target/home-product", `${root}/home`, { recursive: true });
  await cp("target/patchbay-product", `${root}/patchbay`, { recursive: true });
});

test.beforeEach(async () => { entrance = await startStaticProduct(root, "/conduit/"); });
test.afterEach(() => entrance?.child.kill());

test("browser Home retains one exact cross-front receipt", async ({ page }, testInfo) => {
  const carrier = entrance.url.replace(/\/$/, "");
  const steps = ["home.arrived"];
  await page.goto(`${carrier}/home/`);
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
  await expect(page.locator("#host-state")).toHaveText("HELLO, WORLD.");
  await expect(page.locator("#host-state")).toHaveAttribute("data-play-disposition", "completed");
  steps.push("form.run", "play.observed");

  const execution = await page.evaluate(() => globalThis.__conduitHome.evidence());
  expect(execution.effect.host_id).toBe("browser/home");
  expect(execution.effect.boot_id).toMatch(/^browser-boot\/home\/[0-9a-f-]{36}$/);
  expect(execution.receipt.active_play_id).toBe(execution.effect.active_play_id);
  const screenshot = await page.screenshot();

  await command.fill("home");
  await command.press("Enter");
  await page.getByRole("button", { name: "PATCHBAY" }).click();
  await expect(page).toHaveURL(`${carrier}/patchbay/`);
  await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
  steps.push("patchbay.opened");
  await page.goBack();
  await expect(page.getByRole("button", { name: "TOUR" })).toBeVisible();
  steps.push("home.returned");
  expect(steps).toEqual(sharedSteps);

  const front = `browser-${testInfo.project.name}`;
  const artifactName = `${front}.png`;
  const receipt = {
    schema: "conduit.evidence/home-front@1",
    front_id: front,
    proof_class: "live-browser",
    step_ids: steps,
    host_id: execution.effect.host_id,
    boot_id: execution.effect.boot_id,
    plan_id: execution.effect.plan_id,
    active_play_id: execution.effect.active_play_id,
    renderer_id: "browser/home-status-text@1",
    manifestation_id: execution.effect.presentation_id,
    artifact_path: artifactName,
    artifact_sha256: `sha256:${createHash("sha256").update(screenshot).digest("hex")}`,
  };
  const configured = process.env.CONDUIT_HOME_FACE_EVIDENCE_ROOT;
  const output = configured ?? testInfo.outputPath("home-front-evidence");
  await mkdir(output, { recursive: true });
  await writeFile(path.join(output, artifactName), screenshot, { flag: "wx" });
  await writeFile(path.join(output, `${front}.json`), `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx" });
});
