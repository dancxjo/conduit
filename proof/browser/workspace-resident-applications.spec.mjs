import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./static-product-server.mjs";
import { BROWSER_COLD_PREPARATION_MS } from "./browser-preparation.mjs";

let entrance;
const current = page => page.evaluate(() => globalThis.__conduitWorkspace.current());
// Read the retained runtime observation through its packaged, checked bridge.
// This never submits or acknowledges an interaction.
const patchbayObservation = page => page.evaluate(async () => {
  const { bindBrowserRuntimeBridge } = await import(new URL('browser-runtime-bridge.mjs', location.href).href);
  return bindBrowserRuntimeBridge(globalThis.__conduitWorkspace.host.runtime,
    { context: 'Workspace Patchbay proof observation' }).patchbayObservation();
});
const visiblePlotOutput = page => page.locator("[data-plot-output] > output:not([hidden])");

test.beforeEach(async ({ page }) => {
  entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
  await page.setViewportSize({ width: 1280, height: 1000 });
});
test.afterEach(() => entrance?.child.kill());

test("the first body runs and can remove its resident Tutorial Plot without losing its face", async ({ page }) => {
  // Cold Birth/Wake and shelf preparation consume their existing finite budget;
  // retain the ordinary controls and all removal/identity assertions.
  test.setTimeout(20_000 + BROWSER_COLD_PREPARATION_MS);
  await page.goto(entrance.url);
  await expect(page.getByRole("checkbox", { name: "Tutorial", exact: true })).toBeChecked();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  const tutorial = page.locator("[data-body-tutorial]");
  await expect(tutorial).toContainText("Wake this body");
  const born = await current(page);
  await expect(page.getByRole("navigation", { name: "Your plots" }).getByRole("button", { name: "Tutorial", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const resident = tutorial.locator("[data-body-tutorial-resident] > output");
  await expect(resident).toContainText("Try a Plot, inspect what happened, then try again.");
  await expect(resident).toHaveAttribute("data-active-play-id", (await current(page)).active_play_id);
  await expect(tutorial.locator("[data-body-tutorial-core]")).toBeHidden();
  await page.getByRole("button", { name: "+ Plots", exact: true }).click();
  const card = page.locator('[data-application-key^="library-plot-"]').filter({ hasText: /^Tutorial/u });
  await card.getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card).toContainText("Not in your body");
  await page.getByRole("button", { name: "back to the surface", exact: true }).click();
  await expect(tutorial).toBeHidden();
  await expect(page.locator("[data-workspace-surface]")).toBeVisible();
  expect((await current(page)).initial_plots).toHaveLength(born.initial_plots.length - 1);
});

test("resident canonical Patchbay inspects a typed subject and retains an unhandled edit request", async ({ page }, testInfo) => {
  test.setTimeout(20_000 + BROWSER_COLD_PREPARATION_MS);
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Patchbay", exact: true }).check();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const born = await current(page);
  await page.locator("[data-checked-plot-id]").filter({ hasText: "Patchbay" }).click();
  const patchbay = visiblePlotOutput(page);
  const canonical = patchbay.locator("conduit-owner-face");
  await expect(canonical).toBeVisible();
  const inspectButton = canonical.getByRole("button", { name: "Inspect Memory Lantern", exact: true });
  await expect(inspectButton).toBeEnabled();
  const before = await patchbayObservation(page);
  expect(before.body_plan_id).toBe(born.plan_id);
  expect(before.active_play_id).toBe(born.active_play_id);
  expect(before.pending_owner_request).toBeNull();
  const inspect = before.publication.view.actions.find(action => action.name === "Inspect Memory Lantern");
  expect(inspect).toMatchObject({ intent: "patchbay/select-plot@1", availability: "available", arguments: [] });
  await inspectButton.click();
  const form = canonical.locator('[data-owner-action="patchbay.edit.current"]');
  await expect(form).toBeVisible();
  const selected = await patchbayObservation(page);
  const edit = selected.publication.view.actions.find(action => action.identity === "patchbay.edit.current");
  expect(edit).toMatchObject({ intent: "patchbay/edit-subject@1", target: inspect.target, availability: "available" });
  expect(edit.arguments).toHaveLength(1);
  const argument = edit.arguments[0];
  expect(argument).toMatchObject({ name: "subject", value_kind: "value/text" });
  // Choose an actual finite offered member; larger contracts use the same typed
  // text entry over a subject published by this checked Face.
  const subject = argument.choices?.[0]
    ?? selected.publication.view.subjects.find(item => item.role === "Gear")?.identity;
  expect(typeof subject).toBe("string");
  expect(subject.length).toBeGreaterThan(0);
  const input = form.getByLabel(argument.value_name, { exact: true });
  if (await input.evaluate(node => node.tagName === "SELECT")) await input.selectOption(subject);
  else await input.fill(subject);
  const editButton = form.getByRole("button", { name: edit.name, exact: true });
  await expect(editButton).toBeEnabled();
  await editButton.click();
  // Passive observation only. Workspace has no source-editor handler: a typed
  // request remains pending rather than acquiring inferred edit authority.
  await expect.poll(async () => (await patchbayObservation(page)).pending_owner_request).not.toBeNull();
  const pending = await patchbayObservation(page);
  expect(pending.pending_owner_request.request).toMatchObject({ kind: "EditCurrent", subject_identity: subject });
  expect(pending.last_owner_disposition).toEqual(selected.last_owner_disposition);
  expect(pending.body_plan_id).toBe(born.plan_id);
  expect(pending.active_play_id).toBe(born.active_play_id);
  expect((await patchbayObservation(page)).pending_owner_request).toEqual(pending.pending_owner_request);
  const retained = await current(page);
  expect(retained).toMatchObject({ body_id: born.body_id, plan_id: born.plan_id,
    active_play_id: born.active_play_id, workload_revision: born.workload_revision });
  expect(retained.initial_plots).toEqual(born.initial_plots);
  await testInfo.attach("canonical-patchbay-pending-edit.json", {
    body: JSON.stringify({ before, selected, pending, retained,
      boundary: "Workspace retained typed owner request; no source edit or owner completion claimed" }, null, 2),
    contentType: "application/json",
  });
});
