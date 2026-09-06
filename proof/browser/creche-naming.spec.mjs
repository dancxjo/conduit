import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";
import { reviewAndBirth } from "./creche-test-actions.mjs";

let entrance;

test.beforeEach(async () => { entrance = await startCreche(); });
test.afterEach(() => entrance?.child.kill());

test("Crèche suggestions expose diverse structures while remaining editable metadata", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page).toHaveURL(/\/creche\/birth\/$/);
  await expect(page.locator('[data-application-key="workflow"]')).toHaveAttribute("data-application-component", "stepper");
  await expect(page.locator('[data-application-key="workflow"]')).toHaveAttribute("data-application-current", "1");

  const birth = page.locator(".body-birth-runner");
  const name = birth.getByLabel("Friendly Body name");
  const tradition = birth.getByLabel("Naming tradition");
  await expect(birth.locator('[data-application-component="form-field"]')).toHaveCount(4);
  await expect(birth.locator('[data-application-key="initial-forms-field"]')).toHaveAttribute("data-application-component", "choice-group");
  await expect(birth.getByRole("checkbox", { name: "Morse Network" })).not.toBeChecked();
  await birth.getByRole("checkbox", { name: "Memory Lantern" }).check();
  await expect(birth.getByRole("checkbox", { name: "Memory Lantern" })).toBeChecked();
  await expect(birth.locator('[data-application-key="initial-forms-help"]')).toHaveText("1 of 3 reviewed Forms selected; maximum 16.");
  await expect(name).toHaveAttribute("aria-describedby", /description/);
  await expect(birth.getByLabel("Conduit Form source")).toHaveAttribute("aria-describedby", /description/);
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
  await source.evaluate((element) => element.setSelectionRange(0, 4));
  const selectedStyle = await source.evaluate((element) => {
    const style = getComputedStyle(element, "::selection");
    return { color: style.color, fill: style.webkitTextFillColor };
  });
  expect(selectedStyle.color).not.toBe("rgba(0, 0, 0, 0)");
  expect(selectedStyle.fill).not.toBe("rgba(0, 0, 0, 0)");
  await expect(tradition.locator("option")).toHaveCount(24);
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("variation 0");
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("not the Body ID");

  await tradition.selectOption("chinese");
  await expect(name).toHaveValue(/^[\p{Script=Latin}\p{Mark}]+ [\p{Script=Latin}\p{Mark} ]+$/u);
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("Chinese (romanized, family name first)");

  await tradition.selectOption("mexican");
  const mexicanParts = (await name.inputValue()).split(" ").length;
  expect(mexicanParts).toBeGreaterThanOrEqual(3);
  expect(mexicanParts).toBeLessThanOrEqual(4);

  await tradition.selectOption("icelandic");
  await expect(name).toHaveValue(/(?:son|dóttir|bur)$/);

  await tradition.selectOption("ukrainian");
  await expect(name).toHaveValue(/^[\p{Script=Latin}\p{Mark}'-]+ [\p{Script=Latin}\p{Mark}'-]+$/u);
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("Ukrainian (official romanization)");

  await tradition.selectOption("ancient-hebrew");
  await expect(name).toHaveValue(/ (?:ben|bat) /);

  await tradition.selectOption("amharic");
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("Amharic-style patronymic");
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(2);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(3);

  await tradition.selectOption("portuguese");
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("Portuguese multi-surname");
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(3);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(4);

  await tradition.selectOption("tamil");
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("Tamil patronymic forms");
  await expect(name).toHaveValue(/^(?:[A-Z]\. |[\p{Script=Latin}\p{Mark}]+ )[\p{Script=Latin}\p{Mark}]+$/u);

  await tradition.selectOption("indonesian");
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("Indonesian complete personal-name forms");
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(1);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(2);

  await tradition.selectOption("welsh");
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("Welsh modern and patronymic forms");
  expect((await name.inputValue()).split(" ").length).toBeGreaterThanOrEqual(2);
  expect((await name.inputValue()).split(" ").length).toBeLessThanOrEqual(3);

  await tradition.selectOption("kurmanji");
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("Kurdish Kurmanji (Latin script)");
  await expect(name).toHaveValue(/^[\p{Script=Latin}\p{Mark}]+ [\p{Script=Latin}\p{Mark}]+$/u);

  await tradition.selectOption("targus");
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("The TARGUS family");
  await expect(name).toHaveValue("TARGUS TARGUS");
  await birth.getByRole("button", { name: "Suggest another name" }).click();
  await expect(name).toHaveValue("TARGUS TARGUS");

  await tradition.selectOption("kurmanji");
  const slot = birth.locator('[data-application-slot="birth-fields"]');
  const revision = Number(await slot.getAttribute("data-application-revision"));
  await birth.getByRole("button", { name: "Suggest another name" }).click();
  expect(Number(await slot.getAttribute("data-application-revision"))).toBeGreaterThan(revision);
  await expect(birth.locator('[data-application-key="name-origin"]')).toContainText("variation 2");

  await name.fill("Juniper Signalhouse");
  await reviewAndBirth(page, birth);
  await expect(birth.locator('[data-application-key="body-identities"]')).toContainText("Juniper Signalhouse");
  await expect(birth.locator('[data-application-key="body-evidence"]')).toHaveAttribute("data-application-evidence", "succeeded");
  const bodyId = await birth.getAttribute("data-body-id");
  await page.getByRole("button", { name: "2. First Host" }).click();
  await expect(page).toHaveURL(/\/creche\/first-host\/$/);
  await expect(page.locator('[data-application-key="workflow"]')).toHaveAttribute("data-application-current", "2");
  await page.goBack();
  await expect(page).toHaveURL(/\/creche\/birth\/$/);
  const retained = page.locator(".body-birth-runner");
  await expect(retained.getByLabel("Friendly Body name")).toHaveValue("Juniper Signalhouse");
  await expect(retained).toHaveAttribute("data-body-id", bodyId);
  await expect(retained.locator('[data-application-key="name-origin"]')).toContainText("persisted friendly name is metadata");
});

function startCreche() {
  const child = spawn("target/debug/conduit-browser-host", ["--application", "target/creche-product", "--mount", "/creche/", "--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Crèche was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/creche\/)/);
      if (match) { clearTimeout(timeout); resolve({ child, url: match[1] }); }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => { clearTimeout(timeout); reject(new Error(`Crèche exited (${code})\n${output}`)); });
  });
}
