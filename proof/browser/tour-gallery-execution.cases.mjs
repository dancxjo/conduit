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
  const activePlay = (runner) => runner
    .locator('.run-identities [data-application-component="definition"]')
    .filter({ hasText: "Active Play" })
    .locator("dd");

  for (const [title, first, second] of [["Desk Telegraph", "calling", "again"], ["Night Radio", "night report", "later report"]]) {
    test(`Gallery ${title} carries two submissions through one living Play`, async ({ page }) => {
      await openGallery(page);
      const runner = await openForm(page, title);
      await runner.getByRole("button", { name: "Run", exact: true }).click();
      await expect(runner.locator(".gallery-keyboard-input")).toBeFocused();
      await page.keyboard.type(first);
      await page.keyboard.press("Enter");
      await expect(runner.locator(".morse")).toHaveText(first);
      const play = await activePlay(runner).textContent();
      await page.keyboard.type(second);
      await page.keyboard.press("Enter");
      await expect(runner.locator(".morse")).toHaveText(second);
      await expect(activePlay(runner)).toHaveText(play);
      await runner.getByRole("button", { name: "Stop", exact: true }).click();
      await expect(runner.locator('[data-application-key="play-status"]')).toContainText("cancelled");
    });
  }

  test("Gallery Memory Lantern applies typing and Backspace without mutating its source", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Memory Lantern");
    const source = await runner.locator("textarea").inputValue();
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await page.keyboard.type("abc");
    await expect(runner.locator(".morse")).toHaveText("abc");
    const play = await activePlay(runner).textContent();
    await page.keyboard.press("Backspace");
    await expect(runner.locator(".morse")).toHaveText("ab");
    await expect(activePlay(runner)).toHaveText(play);
    await expect(runner.locator("textarea")).toHaveValue(source);
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
  });

  test("Gallery Morse Network reacts to three separated keys in one Play", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Morse Network");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await page.keyboard.press("a");
    await expect(runner.locator(".morse")).toHaveText("·—");
    const play = await activePlay(runner).textContent();
    await page.keyboard.press("b");
    await expect(runner.locator(".morse")).toHaveText("—···");
    await page.keyboard.press("c");
    await expect(runner.locator(".morse")).toHaveText("—·—·");
    await expect(activePlay(runner)).toHaveText(play);
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
  });

  test("Gallery Button Across the Room handles two cycles in one Play", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Button Across the Room");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    const control = runner.getByRole("button", { name: "Hold to light" });
    await control.hover();
    await page.mouse.down();
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator on");
    const play = await activePlay(runner).textContent();
    await page.mouse.up();
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
    await page.mouse.down();
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator on");
    await page.mouse.up();
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
    await expect(activePlay(runner)).toHaveText(play);
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
  });

  test("Gallery Pocket Theremin maps two positions in one living Play", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Pocket Theremin");
    await expect(runner.locator(".structured-output-profile")).toHaveValue("1");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await expect(runner.locator(".input-button")).toBeVisible();
    const bounds = await runner.locator(".input-button").boundingBox();
    expect(bounds).not.toBeNull();
    await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
    await expect(runner.locator(".morse")).toHaveText("10010 Hz");
    const play = await activePlay(runner).textContent();
    await page.mouse.click(bounds.x + bounds.width * 0.75, bounds.y + bounds.height / 2);
    await expect(runner.locator(".morse")).not.toHaveText("10010 Hz");
    await expect(activePlay(runner)).toHaveText(play);
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("cancelled");
    await openForm(page, "Memory Lantern");
    await expect(page.locator(".runner .morse")).toHaveText("ready");
  });

  test("Gallery Firefly Choir reaches pulse five and remains alive until Stop", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Firefly Choir");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await expect(runner.locator(".morse")).toContainText("peer 5");
    await expect(runner.getByRole("button", { name: "Stop", exact: true })).toBeEnabled();
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("cancelled");
  });

  test("Gallery Secret Knock resolves two bounded attempts in one living Play", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Secret Knock");
    await expect(runner.locator(".structured-output-profile")).toHaveValue("3");
    await expect(runner.locator(".compact-patchbay")).toHaveAttribute("data-disposition", "accepted");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    const control = runner.locator(".input-button");
    await expect(control).toBeVisible();
    await runner.locator(".morse").evaluate((output) => {
      output.dataset.presentationCount = "0";
      new MutationObserver(() => {
        output.dataset.presentationCount = String(Number(output.dataset.presentationCount) + 1);
      }).observe(output, { childList: true, characterData: true, subtree: true });
    });
    await control.hover();
    for (const transition of ["down", "up", "down", "up", "down", "up", "down", "up", "down", "up", "down"]) {
      await expect(runner.locator('[data-application-key="play-status"]')).toContainText("button transition");
      await page.mouse[transition]();
    }
    await expect(runner.locator(".morse")).toContainText("matched:");
    await expect.poll(() => runner.locator(".morse").getAttribute("data-presentation-count"))
      .toBe("2");
    await expect(runner.getByRole("button", { name: "Stop", exact: true })).toBeEnabled();
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("cancelled");
  });

  test("Gallery reopens an edited draft without confusing it with the reviewed source", async ({ page }) => {
    await openGallery(page);
    let runner = await openForm(page, "Memory Lantern", true);
    const reviewed = await runner.locator(".compact-patchbay").getAttribute("data-source-document-id");
    const source = await runner.locator("textarea").inputValue();
    const draft = source.replace("maximum-bytes = 256", "maximum-bytes = 128");
    expect(draft).not.toBe(source);
    await runner.locator("textarea").fill(draft);
    await openForm(page, "Morse Network");
    runner = await openForm(page, "Memory Lantern", true);
    await expect(runner.locator("textarea")).toHaveValue(draft);
    await expect(runner.locator(".compact-patchbay")).not.toHaveAttribute("data-source-document-id", reviewed);
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await page.keyboard.type("hello");
    await expect(runner.locator(".morse")).toHaveText("hello");
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await runner.getByRole("button", { name: "Restore canonical source", exact: true }).click();
    await expect(runner.locator("textarea")).toHaveValue(source);
    await expect(runner.locator(".compact-patchbay")).toHaveAttribute("data-source-document-id", reviewed);
  });
}
