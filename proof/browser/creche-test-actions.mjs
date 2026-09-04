export async function reviewAndBirth(page, scope = page.locator(".body-birth-runner")) {
  const initialForm = scope.getByRole("button", { name: "Add Morse Network", exact: true });
  if (await initialForm.isVisible()) await initialForm.click();
  await scope.getByRole("button", { name: "Review workload" }).click();
  await scope.getByRole("button", { name: "Birth Body" }).click();
}
