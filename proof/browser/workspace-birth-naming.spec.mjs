import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./static-product-server.mjs";

let entrance;

test.beforeEach(async () => {
  entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
});
test.afterEach(() => entrance?.child.kill());

test("Workspace Birth suggestions expose diverse structures while remaining editable metadata", async ({ page }) => {
  await page.goto(entrance.url);

  const birth = page.locator(".body-birth-runner");
  const name = birth.getByLabel("Friendly Body name", { exact: true });
  const tradition = birth.getByLabel("Naming tradition", { exact: true });
  await expect(birth.locator("conduit-portable-face")).toHaveCount(1);
  await expect(birth.locator(".portable-face-field")).toHaveCount(3);
  for (const checkbox of await birth.getByRole("checkbox").all()) {
    if (await checkbox.isChecked()) await checkbox.uncheck();
  }
  await expect(birth.getByRole("checkbox", { name: "Morse Network" })).not.toBeChecked();
  await birth.getByRole("checkbox", { name: "Memory Lantern" }).check();
  await expect(birth.getByRole("checkbox", { name: "Memory Lantern" })).toBeChecked();
  await expect(birth.locator('[data-source-status$="/birth-review"]')).toContainText("Starting Plots selected: 1");
  await expect(name).toHaveAttribute("aria-describedby", /\S+/);
  await expect(birth.getByLabel("Conduit Plot source")).toHaveAttribute("aria-describedby", /\S+/);
  await birth.getByText("Details and source", { exact: true }).click();
  const source = birth.getByLabel("Conduit Plot source");
  const syntax = birth.locator('[data-application-syntax="conduit"] .syntax-highlight');
  await expect(source).toHaveAttribute("data-syntax-disposition", "accepted");
  await expect(syntax.locator(".syntax-keyword").first()).toHaveText("plot");
  const editorGeometry = await birth.locator(".syntax-editor").evaluate((editor) => {
    const textarea = editor.querySelector("textarea").getBoundingClientRect();
    const backdrop = editor.querySelector(".syntax-highlight").getBoundingClientRect();
    return {
      left: Math.abs(textarea.left - backdrop.left),
      top: Math.abs(textarea.top - backdrop.top),
      width: Math.abs(textarea.width - backdrop.width),
      height: Math.abs(textarea.height - backdrop.height),
    };
  });
  expect(editorGeometry).toEqual({ left: 0, top: 0, width: 0, height: 0 });
  await expect(source).toHaveCSS("padding", await syntax.evaluate((element) => getComputedStyle(element).padding));
  await source.evaluate((element) => element.setSelectionRange(0, 4));
  const selectedStyle = await source.evaluate((element) => {
    const style = getComputedStyle(element, "::selection");
    return { color: style.color, fill: style.webkitTextFillColor };
  });
  expect(selectedStyle.color).not.toBe("rgba(0, 0, 0, 0)");
  expect(selectedStyle.fill).not.toBe("rgba(0, 0, 0, 0)");
  const selectTradition = async value => {
    await tradition.fill("");
    await expect(birth.getByRole("listbox", { name: "Naming tradition suggestions", exact: true }).getByRole("option")).toHaveCount(24);
    const option = birth.locator(`[role="option"][data-source-value="${value}"]`);
    const sourceLabel = await option.textContent();
    await option.click();
    await birth.getByRole("button", { name: "Use naming tradition", exact: true }).click();
    await expect(tradition).toBeEnabled();
    await expect(tradition).toHaveValue(sourceLabel);
  };

  await selectTradition("chinese");
  await expect(name).toHaveValue(/^[\p{Script=Latin}\p{Mark}]+ [\p{Script=Latin}\p{Mark} ]+$/u);
  await expect(tradition).toHaveValue("Chinese (romanized, family name first)");

  await selectTradition("mexican");
  const mexicanParts = (await name.inputValue()).split(" ").length;
  expect(mexicanParts).toBeGreaterThanOrEqual(3);
  expect(mexicanParts).toBeLessThanOrEqual(4);

  await selectTradition("icelandic");
  await expect(name).toHaveValue(/(?:son|dóttir|bur)$/);

  await selectTradition("ukrainian");
  await expect(name).toHaveValue(/^[\p{Script=Latin}\p{Mark}'-]+ [\p{Script=Latin}\p{Mark}'-]+$/u);
  await expect(tradition).toHaveValue("Ukrainian (official romanization)");

  await selectTradition("ancient-hebrew");
  await expect(name).toHaveValue(/ (?:ben|bat) /);

  await selectTradition("amharic");
  await expect(tradition).toHaveValue(/Amharic-style patronymic/u);
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(2);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(3);

  await selectTradition("portuguese");
  await expect(tradition).toHaveValue(/Portuguese multi-surname/u);
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(3);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(4);

  await selectTradition("tamil");
  await expect(tradition).toHaveValue(/Tamil patronymic plots/u);
  await expect(name).toHaveValue(/^(?:[A-Z]\. |[\p{Script=Latin}\p{Mark}]+ )[\p{Script=Latin}\p{Mark}]+$/u);

  await selectTradition("indonesian");
  await expect(tradition).toHaveValue(/Indonesian complete personal-name plots/u);
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(1);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(2);

  await selectTradition("welsh");
  await expect(tradition).toHaveValue(/Welsh modern and patronymic plots/u);
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(2);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(3);

  await selectTradition("kurmanji");
  await expect(tradition).toHaveValue("Kurdish Kurmanji (Latin script)");
  await expect(name).toHaveValue(/^[\p{Script=Latin}\p{Mark}]+ [\p{Script=Latin}\p{Mark}]+$/u);

  await selectTradition("targus");
  await expect(tradition).toHaveValue("The TARGUS family");
  await expect(name).toHaveValue("TARGUS TARGUS");
  await birth.getByRole("button", { name: "Suggest another name" }).click();
  await expect(name).toHaveValue("TARGUS TARGUS");

  const slot = birth.locator('[data-application-slot="birth-fields"]');
  const traditionRevision = Number(await slot.getAttribute("data-portable-face-revision"));
  await selectTradition("kurmanji");
  await expect(tradition).toHaveValue("Kurdish Kurmanji (Latin script)");
  await expect.poll(async () => Number(await slot.getAttribute("data-portable-face-revision"))).toBeGreaterThan(traditionRevision);
  const revision = Number(await slot.getAttribute("data-portable-face-revision"));
  await birth.getByRole("button", { name: "Suggest another name" }).click();
  await expect.poll(async () => Number(await slot.getAttribute("data-portable-face-revision"))).toBeGreaterThan(revision);

  const birthAction = birth.getByRole("button", { name: "Birth Body", exact: true });
  const review = birth.locator('[data-source-status$="/birth-review"]');
  await expect(name).toHaveAccessibleName("Friendly Body name");
  await expect(tradition).toHaveAccessibleName("Naming tradition");
  const emptyRevision = Number(await slot.getAttribute("data-portable-face-revision"));
  await name.fill("");
  await birth.getByRole("button", { name: "Edit Body name", exact: true }).click();
  await expect(name).toHaveValue("");
  await expect.poll(async () => Number(await slot.getAttribute("data-portable-face-revision"))).toBeGreaterThan(emptyRevision);
  // BirthDraft::selection refuses an empty name with InvalidName; the
  // Source view formats that exact readiness result and withholds Birth.
  await expect(review).toContainText("Name: (empty).");
  await expect(review).toContainText("Birth: unavailable (InvalidName).");
  await expect(birthAction).toBeDisabled();

  const unicodeName = "Éléonore 雪";
  const namedRevision = Number(await slot.getAttribute("data-portable-face-revision"));
  await name.fill(unicodeName);
  await birth.getByRole("button", { name: "Edit Body name", exact: true }).click();
  await expect(name).toHaveValue(unicodeName);
  await expect.poll(async () => Number(await slot.getAttribute("data-portable-face-revision"))).toBeGreaterThan(namedRevision);
  await expect(review).toContainText(`Name: ${unicodeName}.`);
  await expect(review).toContainText("Birth: available.");
  await expect(birthAction).toBeEnabled();
  await birth.getByRole("button", { name: "Review workload", exact: true }).click();
  await birthAction.click();
  await expect(page.locator("[data-body-name]")).toHaveText(unicodeName);
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  const current = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(current.friendly_name).toBe(unicodeName);
  expect(current.body_id).toMatch(/^[0-9a-f]{64}$/u);
  expect(current.body_id).not.toBe(unicodeName);
});
