import { expect, test } from "@playwright/test";
import { BROWSER_COLD_PREPARATION_MS, waitForWorkspacePreparation } from "./browser-preparation.mjs";
import { observeRealAudio } from "./browser-audio-proof.mjs";
import { startStaticProduct } from "./static-product-server.mjs";

test("Crib Wallpaper shows exact photos, rotates, and returns to the same Body", async ({ page }) => {
  test.setTimeout(30_000 + 3 * BROWSER_COLD_PREPARATION_MS);
  const entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
  try {
    await observeRealAudio(page);
    await page.clock.setFixedTime(new Date("2026-11-25T12:00:00Z"));
    await page.goto(entrance.url);
    const photos = await page.evaluate(() => ["red", "green", "blue", "black"].map(color => {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 2;
      const context = canvas.getContext("2d");
      context.fillStyle = color;
      context.fillRect(0, 0, 2, 2);
      return canvas.toDataURL("image/png").split(",")[1];
    }));
    const provider = "https://assets.science.nasa.gov/dynamicimage/assets/science/";
    const cors = { "access-control-allow-origin": "*" };
    await page.route("https://science.nasa.gov/wp-json/wp/v2/apod-basic?**", route => route.fulfill({
      headers: cors,
      json: photos.map((_, index) => ({
        date: `2026-11-${24 - index}`, title: `Wallpaper photo ${index + 1}`,
        media_type: "image", hdurl: `${provider}photo-${index}.png`, copyright: "<a href='https://example.invalid/credit'>Test photographer</a>",
      })),
    }));
    await page.route(`${provider}**`, route => {
      const url = new URL(route.request().url());
      const index = Number(url.pathname.match(/photo-(\d)\.png$/u)?.[1]);
      expect(url.search).toBe("?w=1024&h=1024&fit=clip");
      return route.fulfill({
        headers: cors, contentType: "image/png", body: Buffer.from(photos[index], "base64"),
      });
    });
    const wallpaper = page.getByRole("checkbox", { name: "Wallpaper", exact: true });
    await expect(wallpaper).toBeVisible({ timeout: BROWSER_COLD_PREPARATION_MS });
    await expect(page.getByRole("checkbox", { name: "Startup Chime", exact: true })).toBeChecked();
    await page.getByRole("checkbox", { name: "Local Clock and Weather", exact: true }).check();
    await page.getByRole("checkbox", { name: "Patchbay", exact: true }).check();
    await wallpaper.check();
    await page.getByRole("button", { name: "Birth Body", exact: true }).click();
    await page.getByRole("button", { name: "wake body", exact: true }).click();
    await waitForWorkspacePreparation(page);
    const current = () => page.evaluate(() => globalThis.__conduitWorkspace.current());
    const first = await current();
    await expect.poll(() => page.evaluate(() => globalThis.__cueAudio.ended)).toBe(1);
    await page.getByRole("navigation", { name: "Your plots" })
      .getByRole("button", { name: "Wallpaper", exact: true }).click();
    const output = page.locator('[data-face-kind="face/ambient-wallpaper-observation"]:visible');
    await expect(output.getByRole("img", { name: "Wallpaper photo 1", exact: true })).toBeVisible();
    await expect(output).toContainText("Test photographer");
    await expect(output).not.toContainText("<a href");
    const canvas = output.locator("canvas");
    const readPixel = () => canvas.evaluate(element =>
      Array.from(element.getContext("2d").getImageData(0, 0, 1, 1).data));
    expect(await readPixel()).toEqual([255, 0, 0, 255]);
    const firstIdentity = await canvas.getAttribute("data-identity");
    expect(firstIdentity).toMatch(/^[0-9a-f]{64}$/u);
    await page.clock.setFixedTime(new Date("2026-11-25T12:05:01Z"));
    await expect(output.getByRole("img", { name: "Wallpaper photo 2", exact: true })).toBeVisible();
    await expect(canvas).not.toHaveAttribute("data-identity", firstIdentity);
    expect(await readPixel()).toEqual([0, 128, 0, 255]);
    await page.evaluate(() => globalThis.__conduitWorkspace.settled());
    await page.reload();
    await waitForWorkspacePreparation(page, { surfaceTitle: "Wallpaper" });
    await expect(output.getByRole("img", { name: "Wallpaper photo 1", exact: true })).toBeVisible();
    const returned = await current();
    expect(returned.body_id).toBe(first.body_id);
    expect(returned.host_id).toBe(first.host_id);
    expect(returned.boot_id).not.toBe(first.boot_id);
    const residents = page.getByRole("navigation", { name: "Your plots" });
    await residents.getByRole("button", { name: "Local Clock and Weather", exact: true }).click();
    await expect(page.locator('[data-face-kind="face/ambient-clock-weather"]:visible'))
      .toContainText("America/Los_Angeles");
    const checkedWallpaper = await residents.getByRole("button", { name: "Wallpaper", exact: true })
      .getAttribute("data-checked-plot-id");
    expect(checkedWallpaper).toBeTruthy();
    await residents.getByRole("button", { name: "Patchbay", exact: true }).click();
    const patchbay = page.locator('[data-plot-output] > output:not([hidden]) conduit-owner-face');
    await expect(patchbay).toHaveAttribute("aria-busy", "false");
    const menu = patchbay.locator("details[data-body-menu]");
    await menu.locator(":scope > summary").click();
    const inspect = menu.getByRole("button", { name: "Inspect Wallpaper", exact: true });
    expect(JSON.parse(await inspect.getAttribute("data-owner-focus"))).toEqual([
      returned.body_id, expect.stringMatching(/^patchbay\.plot\.\d+$/u), `plot/${checkedWallpaper}`, "submit",
    ]);
    const face = await patchbay.getAttribute("data-face-id");
    await inspect.click();
    await expect(patchbay).not.toHaveAttribute("data-face-id", face);
    expect((await current()).active_play_id).toBe(returned.active_play_id);
  } finally {
    entrance.child.kill();
  }
});

test("Wallpaper reports provider throttling while the resident clock advances", async ({ page }) => {
  test.setTimeout(30_000 + 2 * BROWSER_COLD_PREPARATION_MS);
  const entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
  try {
    await page.route("https://science.nasa.gov/wp-json/wp/v2/apod-basic?**", route => route.fulfill({
      status: 429,
      headers: { "access-control-allow-origin": "*" },
    }));
    await page.goto(entrance.url);
    const wallpaper = page.getByRole("checkbox", { name: "Wallpaper", exact: true });
    await expect(wallpaper).toBeVisible({ timeout: BROWSER_COLD_PREPARATION_MS });
    await wallpaper.check();
    await page.getByRole("checkbox", { name: "Local Clock and Weather", exact: true }).check();
    await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
    await page.getByRole("button", { name: "Birth Body", exact: true }).click();
    await page.getByRole("button", { name: "wake body", exact: true }).click();
    await waitForWorkspacePreparation(page);
    const residents = page.getByRole("navigation", { name: "Your plots" });
    await residents.getByRole("button", { name: "Wallpaper", exact: true }).click();
    await expect(page.locator('[data-face-kind="face/ambient-wallpaper-observation"]:visible'))
      .toHaveText("Wallpaper unavailable: The photograph provider rate-limited acquisition");
    await residents.getByRole("button", { name: "Local Clock and Weather", exact: true }).click();
    const clock = page.locator('[data-face-kind="face/ambient-clock-weather"]:visible');
    await expect(clock).toContainText("America/Los_Angeles");
    const sequence = await clock.getAttribute("data-observation-sequence");
    expect(sequence).toMatch(/^\d+$/u);
    await expect(clock).not.toHaveAttribute("data-observation-sequence", sequence);
  } finally {
    entrance.child.kill();
  }
});
