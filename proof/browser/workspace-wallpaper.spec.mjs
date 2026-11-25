import { expect, test } from "@playwright/test";
import { BROWSER_COLD_PREPARATION_MS, waitForWorkspacePreparation } from "./browser-preparation.mjs";
import { startStaticProduct } from "./static-product-server.mjs";

test("Crib Wallpaper shows exact photos, rotates, and returns to the same Body", async ({ page }) => {
  test.setTimeout(30_000 + 3 * BROWSER_COLD_PREPARATION_MS);
  const entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
  try {
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
        media_type: "image", hdurl: `${provider}photo-${index}.png`, copyright: "Test photographer",
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
    await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
    await page.getByRole("checkbox", { name: "Wallpaper", exact: true }).check();
    await page.getByRole("button", { name: "Birth Body", exact: true }).click();
    await page.getByRole("button", { name: "wake body", exact: true }).click();
    await waitForWorkspacePreparation(page);
    const current = () => page.evaluate(() => globalThis.__conduitWorkspace.current());
    const first = await current();
    await page.getByRole("navigation", { name: "Your plots" })
      .getByRole("button", { name: "Wallpaper", exact: true }).click();
    const output = page.locator('[data-face-kind="face/ambient-wallpaper-observation"]:visible');
    await expect(output.getByRole("img", { name: "Wallpaper photo 1", exact: true })).toBeVisible();
    await expect(output).toContainText("Test photographer");
    const firstIdentity = await output.locator("canvas").getAttribute("data-identity");
    expect(firstIdentity).toMatch(/^[0-9a-f]{64}$/u);
    await page.clock.setFixedTime(new Date("2026-11-25T12:05:01Z"));
    await expect(output.getByRole("img", { name: "Wallpaper photo 2", exact: true })).toBeVisible();
    await expect(output.locator("canvas")).not.toHaveAttribute("data-identity", firstIdentity);
    await page.evaluate(() => globalThis.__conduitWorkspace.settled());
    await page.reload();
    await waitForWorkspacePreparation(page, { surfaceTitle: "Wallpaper" });
    await expect(output.getByRole("img", { name: "Wallpaper photo 1", exact: true })).toBeVisible();
    const returned = await current();
    expect(returned.body_id).toBe(first.body_id);
    expect(returned.host_id).toBe(first.host_id);
    expect(returned.boot_id).not.toBe(first.boot_id);
  } finally {
    entrance.child.kill();
  }
});
