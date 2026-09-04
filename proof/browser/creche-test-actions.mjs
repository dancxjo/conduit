export async function reviewAndBirth(page, scope = page.locator(".body-birth-runner")) {
  const initialForm = scope.locator('[data-application-key="form-morse_network"]');
  if (await initialForm.isVisible() && !await initialForm.isChecked()) await initialForm.check();
  await scope.getByRole("button", { name: "Review workload" }).click();
  await scope.getByRole("button", { name: "Birth Body" }).click();
}
