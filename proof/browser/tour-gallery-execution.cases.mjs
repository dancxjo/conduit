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
  const runIdentity = (runner, label) => runner.locator(".run-identities")
    .evaluate((root, key) => {
      for (const definition of root.querySelectorAll("[data-application-component=definition]")) {
        const name = definition.querySelector("dt")?.textContent?.trim();
        if (name === key) return definition.querySelector("dd")?.textContent?.trim() ?? "";
      }
      return "";
    }, label);
  const captureRunIdentities = async (runner) => {
    await expect.poll(() => runIdentity(runner, "Checked Form")).not.toBe("");
    await expect.poll(() => runIdentity(runner, "Plan")).not.toBe("");
    await expect.poll(async () => ((await activePlay(runner).textContent()) ?? "").trim()).not.toBe("");
    return {
      checkedFormId: await runIdentity(runner, "Checked Form"),
      planId: await runIdentity(runner, "Plan"),
      playId: ((await activePlay(runner).textContent()) ?? "").trim(),
    };
  };
  const expectSameRunIdentities = async (runner, identities) => {
    await expect.poll(() => runIdentity(runner, "Checked Form")).toBe(identities.checkedFormId);
    await expect.poll(() => runIdentity(runner, "Plan")).toBe(identities.planId);
    await expect(activePlay(runner)).toHaveText(identities.playId);
  };
  const expectStanding = async (runner) => {
    await expect.poll(async () => runner.getAttribute("data-lifecycle-disposition"))
      .toMatch(/^(waiting|quiescent_awaiting_input)$/);
  };
  const expectCancelled = async (runner) => {
    await expect.poll(async () => runner.getAttribute("data-lifecycle-disposition")).toBe("cancelled");
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("cancelled");
    await expect(runner.locator(".run-identities")).toContainText("Terminal Sign");
    await expect(runner.getByRole("button", { name: "Stop", exact: true })).toBeDisabled();
    await expect(runner.getByRole("button", { name: "Run", exact: true })).toBeEnabled();
  };

  for (const [title, first, second] of [["Desk Telegraph", "calling", "again"], ["Night Radio", "night report", "later report"]]) {
    test(`Gallery ${title} carries two submissions through one living Play`, async ({ page }) => {
      await openGallery(page);
      const runner = await openForm(page, title);
      await runner.getByRole("button", { name: "Run", exact: true }).click();
      await expectStanding(runner);
      const identities = await captureRunIdentities(runner);
      expect(identities.checkedFormId).toBeTruthy();
      expect(identities.planId).toBeTruthy();
      expect(identities.playId).toBeTruthy();
      await expect(runner.locator(".gallery-keyboard-input")).toBeFocused();
      await page.keyboard.type(first);
      await page.keyboard.press("Enter");
      await expect(runner.locator(".morse")).toHaveText(first);
      await expectSameRunIdentities(runner, identities);
      await expectStanding(runner);
      await page.waitForTimeout(40);
      await page.keyboard.type(second);
      await page.keyboard.press("Enter");
      await expect(runner.locator(".morse")).toHaveText(second);
      await expectSameRunIdentities(runner, identities);
      await runner.getByRole("button", { name: "Stop", exact: true }).click();
      await expectCancelled(runner);
    });
  }

  test("Gallery Memory Lantern applies typing and Backspace without mutating its source", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Memory Lantern");
    const source = await runner.locator("textarea").inputValue();
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await expectStanding(runner);
    const identities = await captureRunIdentities(runner);
    expect(identities.checkedFormId).toBeTruthy();
    expect(identities.planId).toBeTruthy();
    expect(identities.playId).toBeTruthy();
    await page.keyboard.type("abc");
    await expect(runner.locator(".morse")).toHaveText("abc");
    await expectSameRunIdentities(runner, identities);
    await expectStanding(runner);
    await page.waitForTimeout(40);
    await page.keyboard.press("Backspace");
    await expect(runner.locator(".morse")).toHaveText("ab");
    await expectSameRunIdentities(runner, identities);
    await expectStanding(runner);
    await expect(runner.locator("textarea")).toHaveValue(source);
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expectCancelled(runner);
  });

  test("Gallery Morse Network reacts to S/O/S in one Play", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Morse Network");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await expectStanding(runner);
    const identities = await captureRunIdentities(runner);
    expect(identities.checkedFormId).toBeTruthy();
    expect(identities.planId).toBeTruthy();
    expect(identities.playId).toBeTruthy();
    await page.keyboard.press("s");
    await expect(runner.locator(".morse")).toHaveText("···");
    await expectSameRunIdentities(runner, identities);
    await expectStanding(runner);
    await page.waitForTimeout(40);
    await page.keyboard.press("o");
    await expect(runner.locator(".morse")).toHaveText("———");
    await expectStanding(runner);
    await expectSameRunIdentities(runner, identities);
    await page.waitForTimeout(40);
    await page.keyboard.press("s");
    await expect(runner.locator(".morse")).toHaveText("···");
    await expectSameRunIdentities(runner, identities);
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expectCancelled(runner);
  });

  test("Gallery Button Across the Room handles two cycles in one Play", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Button Across the Room");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    const identities = await captureRunIdentities(runner);
    expect(identities.checkedFormId).toBeTruthy();
    expect(identities.planId).toBeTruthy();
    expect(identities.playId).toBeTruthy();
    const control = runner.getByRole("button", { name: "Hold to light" });
    await control.hover();
    await page.mouse.down();
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator on");
    await expectSameRunIdentities(runner, identities);
    await page.mouse.up();
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
    await expectStanding(runner);
    await expectSameRunIdentities(runner, identities);
    await page.mouse.down();
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator on");
    await page.mouse.up();
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
    await expectSameRunIdentities(runner, identities);
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expectCancelled(runner);
  });

  test("Gallery Pocket Theremin maps two positions in one living Play", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Pocket Theremin");
    await expect(runner.locator(".structured-output-profile")).toHaveValue("1");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    await expect(runner.locator(".input-button")).toBeVisible();
    const identities = await captureRunIdentities(runner);
    expect(identities.checkedFormId).toBeTruthy();
    expect(identities.planId).toBeTruthy();
    expect(identities.playId).toBeTruthy();
    const bounds = await runner.locator(".input-button").boundingBox();
    expect(bounds).not.toBeNull();
    await page.mouse.click(bounds.x + bounds.width * 0.25, bounds.y + bounds.height / 2);
    await expect(runner.locator(".morse")).toHaveText("5015 Hz");
    await expectSameRunIdentities(runner, identities);
    await expectStanding(runner);
    await page.waitForTimeout(40);
    await page.mouse.click(bounds.x + bounds.width * 0.75, bounds.y + bounds.height / 2);
    await expect(runner.locator(".morse")).toHaveText("15005 Hz");
    await expectSameRunIdentities(runner, identities);
    await expectStanding(runner);
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expectCancelled(runner);
    await openForm(page, "Memory Lantern");
    await expect(page.locator(".runner .morse")).toHaveText("ready");
  });

  test("Gallery Firefly Choir reaches pulse five and remains alive until Stop", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Firefly Choir");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    const identities = await captureRunIdentities(runner);
    expect(identities.checkedFormId).toBeTruthy();
    expect(identities.planId).toBeTruthy();
    expect(identities.playId).toBeTruthy();
    await expect.poll(async () => {
      const text = await runner.locator(".morse").textContent() ?? "";
      const matched = text.match(/(?:pulse|peer)\s+(\d+)/i);
      return matched ? Number(matched[1]) : -1;
    }).toBeGreaterThanOrEqual(4);
    await expectSameRunIdentities(runner, identities);
    await expectStanding(runner);
    await expect.poll(async () => {
      const text = await runner.locator(".morse").textContent() ?? "";
      const matched = text.match(/(?:pulse|peer)\s+(\d+)/i);
      return matched ? Number(matched[1]) : -1;
    }).toBeGreaterThanOrEqual(5);
    await expectSameRunIdentities(runner, identities);
    await expect.poll(async () => {
      const text = await runner.locator(".morse").textContent() ?? "";
      const matched = text.match(/(?:pulse|peer)\s+(\d+)/i);
      return matched ? Number(matched[1]) : -1;
    }).toBeGreaterThanOrEqual(6);
    await expectSameRunIdentities(runner, identities);
    await expect(runner.getByRole("button", { name: "Stop", exact: true })).toBeEnabled();
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expectCancelled(runner);
  });

  test("Gallery Secret Knock resolves two bounded attempts in one living Play", async ({ page }) => {
    await openGallery(page);
    const runner = await openForm(page, "Secret Knock");
    await expect(runner.locator(".structured-output-profile")).toHaveValue("3");
    await expect(runner.locator(".compact-patchbay")).toHaveAttribute("data-disposition", "accepted");
    await runner.getByRole("button", { name: "Run", exact: true }).click();
    const identities = await captureRunIdentities(runner);
    expect(identities.checkedFormId).toBeTruthy();
    expect(identities.planId).toBeTruthy();
    expect(identities.playId).toBeTruthy();
    const control = runner.locator(".input-button");
    await expect(control).toBeVisible();
    await runner.locator(".morse").evaluate((output) => {
      output.dataset.presentationCount = "0";
      new MutationObserver(() => {
        output.dataset.presentationCount = String(Number(output.dataset.presentationCount) + 1);
      }).observe(output, { childList: true, characterData: true, subtree: true });
    });
    await control.hover();
    for (const transition of ["down", "up", "down", "up", "down", "up"]) {
      await expect(runner.locator('[data-application-key="play-status"]')).toContainText("button transition");
      await page.mouse[transition]();
    }
    await expect.poll(() => runner.locator(".morse").getAttribute("data-presentation-count"))
      .toBe("1");
    const firstAttempt = await runner.locator(".morse").textContent();
    expect(firstAttempt).toContain("matched:");
    await expectSameRunIdentities(runner, identities);
    await expectStanding(runner);
    for (const transition of ["down", "up", "down", "up", "down"]) {
      await expect(runner.locator('[data-application-key="play-status"]')).toContainText("button transition");
      await page.mouse[transition]();
    }
    await expect.poll(() => runner.locator(".morse").getAttribute("data-presentation-count"))
      .toBe("2");
    const secondAttempt = await runner.locator(".morse").textContent();
    expect(secondAttempt).toContain("matched:");
    await expectSameRunIdentities(runner, identities);
    await expectStanding(runner);
    await expect(runner.getByRole("button", { name: "Stop", exact: true })).toBeEnabled();
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expectCancelled(runner);
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
