import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";
import { selectBirthForm } from "./creche-test-actions.mjs";

let entrance;

test.beforeEach(async () => {
  entrance = await startStaticProduct("target/creche-product", "/conduit/creche/");
});
test.afterEach(() => entrance?.child.kill());

test("Crèche composes, persists, reviews, and births three exact initial Forms", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  let birth = page.locator(".body-birth-runner");
  await expect(birth.getByRole("button", { name: "Birth Body" })).toBeDisabled();

  const search = birth.getByLabel("Search Forms");
  await search.fill("lantern");
  await expect(birth.getByLabel("Memory Lantern", { exact: true })).toBeVisible();
  await expect(birth.getByLabel("Morse Network", { exact: true })).toHaveCount(0);
  await search.fill("");
  for (const [index, title] of ["Morse Network", "Memory Lantern", "Desk Telegraph"].entries()) {
    await selectBirthForm(birth, title);
    await expect(birth.locator('[data-application-key="initial-forms-help"]')).toContainText(
      `${index + 1} of 5 reviewed Forms selected`,
    );
  }
  const selectedSource = birth.getByLabel("Selected Conduit Form source");
  await expect(selectedSource).toHaveValue(/form morse_network/);
  await expect(selectedSource).toHaveValue(/form memory_lantern/);
  await expect(selectedSource).toHaveValue(/form desk_telegraph/);
  await expect(selectedSource).not.toHaveValue(/conduit\.creche\/reviewed-form-bundle/);
  await expect(birth.locator('[data-application-key="initial-forms-help"]')).toHaveText(
    "3 of 5 reviewed Forms selected; maximum 16.",
  );
  await birth.getByRole("button", { name: "Review workload" }).click();
  await expect(birth.getByRole("button", { name: "Birth Body" })).toBeEnabled();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  birth = page.locator(".body-birth-runner");
  await expect(birth.getByRole("button", { name: "Birth Body" })).toBeDisabled();
  await expect(birth.locator('[data-application-key="review-basis"]')).toContainText("not reviewed");
  for (const title of ["Morse Network", "Memory Lantern", "Desk Telegraph"]) {
    await expect(birth.getByLabel(title, { exact: true })).toBeChecked();
  }
  await selectBirthForm(birth, "Desk Telegraph", false);
  await expect(birth.getByLabel("Desk Telegraph", { exact: true })).not.toBeChecked();
  await selectBirthForm(birth, "Desk Telegraph");

  await birth.getByRole("button", { name: "Review workload" }).click();
  await expect(birth.locator('[data-application-key="review-basis"]')).toContainText(
    "current Host OFFER(s); no permission or resource acquired; no Body Plan or Play created",
  );
  await expect(birth.locator('[data-application-key="birth-status"]')).toContainText(
    "Review accepted 3 Form(s)",
  );
  await expect(birth.getByRole("button", { name: "Birth Body" })).toBeEnabled();
  await birth.getByRole("button", { name: "Birth Body" }).click();

  await expect(birth.locator('[data-application-key="initial-forms"]')).toContainText(
    "morse_network, memory_lantern, desk_telegraph",
  );
  await expect(birth.locator('[data-application-key="workload-revision"] dd')).toHaveText("0");
  const receipt = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    const code = api.conduit_creche_current();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return { code, value: JSON.parse(new TextDecoder().decode(bytes)) };
  });
  expect(receipt.code).toBe(0);
  expect(receipt.value.initial_forms).toHaveLength(3);
  expect(receipt.value.workload_revision).toBe(0);
  expect(receipt.value.initial_forms.every((form) => form.source_document_id && form.checked_form_id)).toBe(true);
  expect(receipt.value.initial_review).toMatchObject({
    body_plan_created: false,
    play_created: false,
    resources_acquired: false,
    authority_acquired: false,
  });
});

test("Crèche accepts exact Gallery handoff and visibly refuses stale restored identity", async ({ page }) => {
  await page.goto(entrance.url);
  const birth = page.locator(".body-birth-runner");
  await selectBirthForm(birth, "Morse Network");
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());
  const selected = await page.evaluate(async () => (
    await globalThis.__conduitBrowserApplication.storage.readJson("form-selection")
  ).forms[0]);
  await page.evaluate(() => globalThis.__conduitBrowserApplication.storage.deleteJson("form-selection"));

  const handoff = new URL(entrance.url);
  handoff.searchParams.set("form", selected.name);
  handoff.searchParams.set("source_document_id", selected.source_document_id);
  handoff.searchParams.set("checked_form_id", selected.checked_form_id);
  await page.goto(handoff.href);
  await expect(page.getByLabel("Morse Network", { exact: true })).toBeChecked();
  await expect(page.locator('[data-application-key="birth-status"]')).toContainText(
    "Morse Network was revalidated and preselected from Gallery",
  );
  await expect(page.locator('[data-application-key="birth-status"]')).toContainText(
    "no Body has been born",
  );

  await page.evaluate(async () => {
    await globalThis.__conduitBrowserApplication.storage.writeJson("form-selection", {
      schema: "conduit.creche/form-selection@1",
      inventory_source_document_id: "stale-source",
      forms: [{ name: "morse_network", source_document_id: "stale-source", checked_form_id: "stale-check" }],
    });
  });
  await page.goto(new URL("birth/", entrance.url).href);
  await expect(page.locator('[data-application-key="birth-status"]')).toContainText(
    "1 stale or over-capacity restored Form selection(s) were refused",
  );
  await expect(page.getByLabel("Morse Network", { exact: true })).not.toBeChecked();
});
