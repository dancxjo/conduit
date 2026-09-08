import { expect, test } from "@playwright/test";

export function registerTourGalleryExecutionTests(openStep) {
  const openForm = async (page, title, inspect = false) => {
    const card = page.locator('[data-application-key="gallery-cards"] > [data-application-component="panel"]')
      .filter({ has: page.getByRole("heading", { name: title, exact: true }) });
    await card.getByRole("button", { name: inspect ? "Inspect Patchbay" : "Open in laboratory", exact: true }).click();
    return page.locator(".tour-workbench .runner");
  };
  const openGallery = async (page) => {
    await openStep(page, 0);
    await page.getByRole("button", { name: "Form Gallery", exact: true }).click();
  };

  for (const [title, output] of [["Desk Telegraph", "CALLING"], ["Night Radio", "NIGHT REPORT"]]) {
    test(`Gallery ${title} carries its message through the real local composition`, async ({ page }) => {
      await openGallery(page);
      const runner = await openForm(page, title);
      await runner.getByRole("button", { name: "Run", exact: true }).click();
      await expect(runner.locator(".morse")).toHaveText(output);
      await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Quiescent");
      await runner.getByRole("button", { name: "Stop", exact: true }).click();
      await expect(runner.locator('[data-application-key="play-status"]')).toContainText("cancelled");
    });
  }

  test("Gallery message control edits and runs the actual source", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Memory Lantern");
    await runner.getByRole("textbox", { name: "Your message", exact: true }).fill("HELLO THERE");
    await expect(runner.locator("textarea")).toHaveValue(/text\/literal\("HELLO THERE"\)/);
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await expect(runner.locator(".morse")).toHaveText("HELLO THERE");
    await runner.getByText("Open the wiring & source", { exact: true }).click();
    await runner.getByRole("button", { name: "Restore canonical source", exact: true }).click();
    await expect(runner.getByRole("textbox", { name: "Your message", exact: true })).toHaveValue("READY");
  });

  test("Gallery Pocket Theremin completes a real pointer acquisition and can cancel another", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Pocket Theremin");
    await expect(runner.locator(".structured-output-profile")).toHaveValue("1");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await expect(runner.locator(".input-button")).toBeVisible();
    const bounds = await runner.locator(".input-button").boundingBox();
    expect(bounds).not.toBeNull();
    await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
    await expect(runner.locator(".morse")).toHaveText("10010 Hz");
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("cancelled");
    await openForm(page, "Memory Lantern");
    await expect(page.locator(".runner .morse")).toHaveText("ready");
  });

  test("Gallery Firefly Choir completes four pulses through the production kernel", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Firefly Choir");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("8 presentations");
    await expect(runner.locator(".morse")).toContainText("peer 4");
  });

  test("Gallery Secret Knock admits its pattern profile and resolves a bounded attempt", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Secret Knock");
    await expect(runner.locator(".structured-output-profile")).toHaveValue("3");
    await expect(runner.locator(".compact-patchbay")).toHaveAttribute("data-disposition", "accepted");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    const control = runner.locator(".input-button");
    await expect(control).toBeVisible();
    await control.click();
    await control.click();
    await control.hover();
    await page.mouse.down();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
    await page.mouse.up();
    await expect(runner.locator(".morse")).not.toHaveText("ready");
  });

  test("Gallery reopens an edited draft without confusing it with the reviewed source", async ({ page }) => {
    await openGallery(page);
    let runner = await openForm(page, "Memory Lantern", true);
    const reviewed = await runner.locator(".compact-patchbay").getAttribute("data-source-document-id");
    const source = await runner.locator("textarea").inputValue();
    const draft = source.replace("READY", "HELLO");
    expect(draft).not.toBe(source);
    await runner.locator("textarea").fill(draft);
    await openForm(page, "Morse Network");
    runner = await openForm(page, "Memory Lantern", true);
    await expect(runner.locator("textarea")).toHaveValue(draft);
    await expect(runner.locator(".compact-patchbay")).not.toHaveAttribute("data-source-document-id", reviewed);
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await expect(runner.locator(".morse")).toHaveText("HELLO");
    await runner.getByRole("button", { name: "Restore canonical source", exact: true }).click();
    await expect(runner.locator("textarea")).toHaveValue(source);
    await expect(runner.locator(".compact-patchbay")).toHaveAttribute("data-source-document-id", reviewed);
  });
}
