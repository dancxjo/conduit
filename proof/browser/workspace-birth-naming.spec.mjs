import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;

test.beforeEach(async () => {
  entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
});
test.afterEach(() => entrance?.child.kill());

test("Workspace Birth suggestions expose diverse structures while remaining editable metadata", async ({ page }) => {
  await page.goto(entrance.url);

  const birth = page.locator(".body-birth-runner");
  const name = birth.getByLabel("Friendly Body name");
  const tradition = birth.getByLabel("Naming tradition");
  await expect(birth.locator('[data-application-component="form-field"]')).toHaveCount(4);
  await expect(birth.locator('[data-application-key="initial-forms"]')).toHaveAttribute("data-application-component", "choice-group");
  for (const checkbox of await birth.getByRole("checkbox").all()) {
    if (await checkbox.isChecked()) await checkbox.uncheck();
  }
  await expect(birth.getByRole("checkbox", { name: "Morse Network" })).not.toBeChecked();
  await birth.getByRole("checkbox", { name: "Memory Lantern" }).check();
  await expect(birth.getByRole("checkbox", { name: "Memory Lantern" })).toBeChecked();
  await expect(birth.locator('[data-application-key="selected-forms"]')).toHaveText("Selected: 1");
  await expect(name).toHaveAttribute("aria-describedby", /\S+/);
  await expect(birth.getByLabel("Conduit Form source")).toHaveAttribute("aria-describedby", /\S+/);
  await birth.getByText("Details and source", { exact: true }).click();
  const source = birth.getByLabel("Conduit Form source");
  const syntax = birth.locator('[data-application-syntax="conduit"] .syntax-highlight');
  await expect(source).toHaveAttribute("data-syntax-disposition", "accepted");
  await expect(syntax.locator(".syntax-keyword").first()).toHaveText("form");
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
  await expect(tradition.locator("option")).toHaveCount(24);

  await tradition.selectOption("chinese");
  await expect(name).toHaveValue(/^[\p{Script=Latin}\p{Mark}]+ [\p{Script=Latin}\p{Mark} ]+$/u);
  await expect(tradition.locator("option:checked")).toContainText("Chinese (romanized, family name first)");

  await tradition.selectOption("mexican");
  const mexicanParts = (await name.inputValue()).split(" ").length;
  expect(mexicanParts).toBeGreaterThanOrEqual(3);
  expect(mexicanParts).toBeLessThanOrEqual(4);

  await tradition.selectOption("icelandic");
  await expect(name).toHaveValue(/(?:son|dóttir|bur)$/);

  await tradition.selectOption("ukrainian");
  await expect(name).toHaveValue(/^[\p{Script=Latin}\p{Mark}'-]+ [\p{Script=Latin}\p{Mark}'-]+$/u);
  await expect(tradition.locator("option:checked")).toContainText("Ukrainian (official romanization)");

  await tradition.selectOption("ancient-hebrew");
  await expect(name).toHaveValue(/ (?:ben|bat) /);

  await tradition.selectOption("amharic");
  await expect(tradition.locator("option:checked")).toContainText("Amharic-style patronymic");
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(2);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(3);

  await tradition.selectOption("portuguese");
  await expect(tradition.locator("option:checked")).toContainText("Portuguese multi-surname");
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(3);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(4);

  await tradition.selectOption("tamil");
  await expect(tradition.locator("option:checked")).toContainText("Tamil patronymic forms");
  await expect(name).toHaveValue(/^(?:[A-Z]\. |[\p{Script=Latin}\p{Mark}]+ )[\p{Script=Latin}\p{Mark}]+$/u);

  await tradition.selectOption("indonesian");
  await expect(tradition.locator("option:checked")).toContainText("Indonesian complete personal-name forms");
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(1);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(2);

  await tradition.selectOption("welsh");
  await expect(tradition.locator("option:checked")).toContainText("Welsh modern and patronymic forms");
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(2);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(3);

  await tradition.selectOption("kurmanji");
  await expect(tradition.locator("option:checked")).toContainText("Kurdish Kurmanji (Latin script)");
  await expect(name).toHaveValue(/^[\p{Script=Latin}\p{Mark}]+ [\p{Script=Latin}\p{Mark}]+$/u);

  await tradition.selectOption("targus");
  await expect(tradition.locator("option:checked")).toContainText("The TARGUS family");
  await expect(name).toHaveValue("TARGUS TARGUS");
  await birth.getByRole("button", { name: "Suggest another name" }).click();
  await expect(name).toHaveValue("TARGUS TARGUS");

  await tradition.selectOption("kurmanji");
  const slot = birth.locator('[data-application-slot="birth-fields"]');
  const revision = Number(await slot.getAttribute("data-application-revision"));
  await birth.getByRole("button", { name: "Suggest another name" }).click();
  expect(Number(await slot.getAttribute("data-application-revision"))).toBeGreaterThan(revision);

  await name.fill("Juniper Signalhouse");
  await birth.getByRole("button", { name: "Review workload", exact: true }).click();
  await birth.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-body-name]")).toHaveText("Juniper Signalhouse");
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  const current = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(current.friendly_name).toBe("Juniper Signalhouse");
  expect(current.body_id).toMatch(/^[0-9a-f]{64}$/u);
  expect(current.body_id).not.toBe("Juniper Signalhouse");
});
