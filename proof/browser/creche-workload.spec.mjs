import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { startStaticProduct } from "./tour-test-server.mjs";
import { selectBirthForm, reviewAndBirth, openCrecheStep } from "./creche-test-actions.mjs";

let entrance;

test.beforeEach(async () => {
  entrance = await startStaticProduct("target/creche-product", "/conduit/creche/");
});
test.afterEach(() => entrance?.child.kill());

test("shared Crèche birth surface preserves naming, search, and exact selection", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const birth = page.locator(".body-birth-runner");
  await expect(birth.getByRole("heading", { name: "A Body of your own" })).toBeVisible();
  await birth.getByLabel("Naming tradition", { exact: true }).selectOption("roman");
  await birth.getByLabel("Friendly Body name", { exact: true }).fill("Juniper");
  await birth.getByRole("checkbox", { name: "Memory Lantern", exact: true }).check();
  const search = birth.getByLabel("Search Forms", { exact: true });
  await search.fill("no-such-form");
  await expect(birth.locator('[data-application-key="initial-forms"]')).toHaveText("No Forms match your search. — Your selected Forms are still included.");
  await expect(birth.getByText("Selected: 1", { exact: true })).toBeVisible();
  await search.fill("");
  await expect(birth.getByRole("checkbox", { name: "Memory Lantern", exact: true })).toBeChecked();
  await expect(birth.getByLabel("Friendly Body name", { exact: true })).toHaveValue("Juniper");
  await page.screenshot({ path: testInfo.outputPath("browser-birth.png"), fullPage: true });
  await birth.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(birth).toHaveAttribute("data-body-id", /\S+/);
  await expect(birth.locator('[data-application-key="body-identities"]')).toContainText("Juniper");
  await page.screenshot({ path: testInfo.outputPath("browser-born.png"), fullPage: true });
  await birth.getByRole("button", { name: "Continue on this Host", exact: true }).click();
  await expect(page.getByRole("button", { name: "Give this Body its first Host", exact: true })).toBeVisible();
});

test("Crèche composes, persists, reviews, and births three exact initial Forms", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  let birth = page.locator(".body-birth-runner");
  await expect(birth.getByRole("button", { name: "Birth Body" })).toBeEnabled();

  const search = birth.getByLabel("Search Forms");
  await search.fill("lantern");
  await expect(birth.getByLabel("Memory Lantern", { exact: true })).toBeVisible();
  await expect(birth.getByLabel("Morse Network", { exact: true })).toHaveCount(0);
  await search.fill("");
  for (const [index, title] of ["Morse Network", "Memory Lantern", "Desk Telegraph"].entries()) {
    await selectBirthForm(birth, title);
    await expect(birth.locator('[data-application-key="selected-forms"]')).toContainText(
      `Selected: ${index + 1}`,
    );
  }
  await birth.getByText("Details and source", { exact: true }).click();
  const selectedSource = birth.getByLabel("Selected Conduit Form source");
  await expect(selectedSource).toHaveValue(/form morse_network/);
  await expect(selectedSource).toHaveValue(/form memory_lantern/);
  await expect(selectedSource).toHaveValue(/form desk_telegraph/);
  await expect(selectedSource).not.toHaveValue(/conduit\.creche\/reviewed-form-bundle/);
  await expect(birth.locator('[data-application-key="selected-forms"]')).toHaveText(
    "Selected: 3",
  );
  await birth.getByRole("button", { name: "Review workload" }).click();
  await expect(birth.getByRole("button", { name: "Birth Body" })).toBeEnabled();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  birth = page.locator(".body-birth-runner");
  await expect(birth.getByRole("button", { name: "Birth Body" })).toBeEnabled();
  await birth.getByText("Details and source", { exact: true }).click();
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
    "Ready to birth with 3 Form(s)",
  );
  await expect(birth.getByRole("button", { name: "Birth Body" })).toBeEnabled();
  await birth.getByRole("button", { name: "Birth Body" }).click();

  await expect(birth.locator('[data-application-slot="birth-evidence"] [data-application-key="initial-forms"]')).toContainText(
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

test("Crèche births canonical button, clock, and unrelated Forms without source edits", async ({ page }) => {
  const bundled = await page.request.get(new URL("forms/initial-body.conduit", entrance.url).href);
  expect(bundled.ok()).toBe(true);
  const inventory = await bundled.json();
  for (const slug of ["button-across-room", "clock", "desk-telegraph"]) {
    const canonical = readFileSync(new URL(`../../forms/${slug}/main.conduit`, import.meta.url), "utf8");
    expect(inventory.forms.find((form) => form.slug === slug)?.source).toBe(canonical);
  }
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const birth = page.locator(".body-birth-runner");
  for (const title of ["Button Across Room", "Clock-demo", "Desk Telegraph"]) {
    await selectBirthForm(birth, title);
  }
  await expect(birth.locator('[data-application-key="selected-forms"]')).toHaveText(
    "Selected: 3",
  );
  await birth.getByText("Details and source", { exact: true }).click();
  const source = birth.getByLabel("Selected Conduit Form source");
  await expect(source).toHaveValue(/form button_across_room/);
  await expect(source).toHaveValue(/form clock-demo/);
  await expect(source).toHaveValue(/form desk_telegraph/);
  await birth.getByRole("button", { name: "Review workload" }).click();
  await expect(birth.getByRole("button", { name: "Birth Body" })).toBeEnabled();
  await birth.getByRole("button", { name: "Birth Body" }).click();
  const receipt = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    const code = api.conduit_creche_current();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return { code, value: JSON.parse(new TextDecoder().decode(bytes)) };
  });
  expect(receipt.code).toBe(0);
  expect(receipt.value.initial_forms.map((form) => form.name).sort()).toEqual(
    ["button_across_room", "clock-demo", "desk_telegraph"].sort(),
  );
  expect(receipt.value.workload_revision).toBe(0);
  expect(receipt.value.initial_review).toMatchObject({
    selected_form_count: 3, body_plan_created: false, play_created: false,
    resources_acquired: false, authority_acquired: false,
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

test("the standalone Crèche runs the same durable birth and graduation path without Tour assets", async ({ page }) => {
  const responses = [];
  page.on("response", (response) => responses.push(new URL(response.url()).pathname));
  await page.goto(entrance.url);
  await expect(page).toHaveTitle("Conduit Crèche");
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const birth = page.locator(".body-birth-runner");
  await expect(birth.locator('[data-application-key="initial-forms"]')).toHaveAttribute("data-application-component", "choice-group");
  await expect(birth.getByRole("checkbox", { name: "Morse Network" })).not.toBeChecked();
  await expect(birth.getByRole("checkbox", { name: "Memory Lantern" })).not.toBeChecked();
  await birth.getByLabel("Friendly Body name").fill("standalone firefly");
  await reviewAndBirth(page, birth);
  const bodyId = await birth.getAttribute("data-body-id");
  expect(bodyId).toMatch(/^[0-9a-f]{64}$/);
  await expect(page.locator('.creche-body-context [data-application-component="panel"]')).toContainText("standalone firefly");
  await openCrecheStep(page, "2. First Host");
  await expect(page.locator('[data-application-key="attach-host"]')).toHaveAttribute("data-application-action", "host.attach");
  await page.getByRole("button", { name: "Give this Body its first Host" }).click();
  await expect(page.locator('.first-host-runner [data-application-key="host-status"]')).toHaveAttribute("data-application-component", "success-status");
  await expect(page.locator('.first-host-runner [data-application-key="host-evidence"]')).toHaveAttribute("data-application-evidence", "succeeded");
  await openCrecheStep(page, "4. Graduate");
  const finishWithoutPatchbay = page.getByRole("button", { name: "Finish without hosted Patchbay", exact: true });
  await expect(finishWithoutPatchbay).toHaveAttribute("data-application-action", "graduate.without-patchbay");
  await finishWithoutPatchbay.click();
  await expect(page.locator('[data-application-key="graduation-status"]')).toHaveAttribute("data-application-component", "success-status");
  await expect(page.getByRole("button", { name: "End the Crèche", exact: true })).toHaveAttribute("data-application-action", "graduate.end");
  await expect(page.locator(".graduation-runner")).toHaveAttribute("data-body-id", bodyId);
  await expect(page.locator('.body-biography [data-application-key^="biography-record-"]')).toHaveCount(4);
  const durable = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    api.conduit_creche_biography();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return JSON.parse(new TextDecoder().decode(bytes));
  });
  expect(durable.body_id).toBe(bodyId);
  expect(durable.schema).toBe("conduit.body/biography-evidence@2");
  const crecheRuntimeExports = await page.evaluate(() => Object.keys(globalThis.__conduitCrecheHost.runtime));
  expect(crecheRuntimeExports.some((name) => name.startsWith("conduit_tour_"))).toBe(false);
  expect((await page.request.get(new URL("/tour/", entrance.url).href)).status()).toBe(404);
  expect(responses.some((path) => path.startsWith("/tour/") || path.includes("chapter-"))).toBe(false);
});

test("the standalone Crèche birth controls remain separated at a narrow viewport", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const runner = page.locator(".body-birth-runner");
  const [program, name, source, editor] = await Promise.all([
    runner.locator('[data-application-slot="birth-fields"] [data-application-key="initial-forms"]').boundingBox(),
    runner.getByLabel("Friendly Body name").boundingBox(),
    runner.locator(".birth-presentation .birth-details").boundingBox(),
    runner.locator(".birth-presentation").boundingBox(),
  ]);
  for (const box of [program, name, source, editor]) expect(box).not.toBeNull();
  expect(name.y + name.height).toBeLessThanOrEqual(program.y);
  expect(program.y + program.height).toBeLessThanOrEqual(source.y);
  for (const control of [program, name, source]) {
    expect(control.x).toBeGreaterThanOrEqual(editor.x);
    expect(control.x + control.width).toBeLessThanOrEqual(editor.x + editor.width);
  }
  await runner.getByText("Details and source", { exact: true }).click();
  await expect(runner.getByLabel("Selected Conduit Form source", { exact: true })).toBeVisible();
  const selectAppearance = await runner.getByLabel("Naming tradition").evaluate(
    (element) => getComputedStyle(element).appearance,
  );
  expect(selectAppearance).toBe("none");
});
