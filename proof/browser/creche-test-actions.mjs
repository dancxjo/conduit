import { expect } from "@playwright/test";

export async function reviewAndBirth(page, scope = page.locator(".body-birth-runner")) {
  const initialForm = scope.getByRole("checkbox", { name: "Morse Network", exact: true });
  if (await initialForm.isVisible() && !await initialForm.isChecked()) await initialForm.check();
  await scope.getByRole("button", { name: "Birth Body" }).click();
}

export async function selectBirthForm(scope, title, selected = true) {
  await expect(scope.getByRole("button", { name: "Suggest another name" })).toBeEnabled();
  const choice = scope.getByLabel(title, { exact: true });
  if (selected) await choice.check();
  else await choice.uncheck();
}

export async function openCrecheStep(page, name) {
  const options = page.locator(".creche-options");
  if (await options.getAttribute("open") === null) {
    await options.locator("summary").click();
  }
  await options.getByRole("button", { name, exact: true }).click();
}
