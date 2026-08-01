import { expect, test } from "@playwright/test";

test("distinguishes clock event control audio and retained-state faceplate ports", async ({ page }) => {
  await page.goto("/tour/public/index.html?lesson=library.bounded-audio-processing");
  await expect(page.locator("html")).toHaveAttribute(
    "data-tour-ready",
    "true",
    { timeout: 20_000 },
  );

  for (const family of ["clock", "event", "control", "audio", "state"]) {
    const row = page.locator(`.faceplate-port-row[data-signal-family="${family}"]`).first();
    await expect(row, `${family} port is projected`).toBeVisible();
    await expect(row.locator(".jack-label")).toHaveAttribute(
      "title",
      new RegExp(`${family} signal`),
    );
    await expect(row.locator(".jack-handle")).toHaveAttribute(
      "data-signal-family",
      family,
    );
  }

  await expect(page.locator(".patchbay-cord.type-family-event").first()).toBeVisible();
  await expect(page.locator(".patchbay-cord.type-family-control").first()).toBeVisible();
  await expect(page.locator(".patchbay-cord.type-family-audio").first()).toBeVisible();

  const eventJack = page.locator('.jack-handle[data-signal-family="event"]').first();
  const controlJack = page.locator('.jack-handle[data-signal-family="control"]').first();
  const audioJack = page.locator('.jack-handle[data-signal-family="audio"]').first();
  const stateJack = page.locator('.jack-handle[data-signal-family="state"]').first();
  await expect(eventJack).toHaveCSS("border-radius", "0px");
  await expect(controlJack).toHaveCSS("border-radius", "50%");
  await expect(audioJack).toHaveCSS("border-radius", "999px");
  await expect(stateJack).toHaveCSS("border-radius", "0px");
});

test("states the standing-patch contrast with an imperative loop", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await expect(page.locator("#cover-projects")).toContainText("imperative loop");
  await expect(page.locator("#cover-projects")).toContainText("pulses advance state");
  await expect(page.locator("#cover-projects")).toContainText("lifecycle control");
});
