import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium, expect } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

const [publication, output] = process.argv.slice(2);
if (!publication || !output) throw new Error("expected publication root and new review directory");
await mkdir(output); // Retain prior review outputs; never overwrite them.
const entrance = await startStaticProduct(publication, "/journey/");
let browser;
try {
  browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const failures = [];
  page.on("pageerror", error => failures.push(error.message));
  page.on("response", response => { if (response.status() >= 400) failures.push(`${response.status()} ${response.url()}`); });
  await page.goto(entrance.url);
  await expect(page.getByRole("heading", { name: "One journey. Three bodies." })).toBeVisible();
  await expect(page.locator("pre:visible")).toHaveCount(0);
  await expect(page.locator("details[open]")).toHaveCount(0);
  await expect(page.locator(".chapter")).toHaveCount(13);
  await expect(page.locator(".body-panel:visible")).toHaveCount(39);
  await expect(page.locator("[data-transcript] a")).toHaveCount(0);
  await page.screenshot({ path: join(output, "desktop-entry.png") });
  const images = page.locator(".chapter img");
  expect(await images.count()).toBe(37);
  for (const image of await images.all()) {
    await image.scrollIntoViewIfNeeded();
    await expect.poll(() => image.evaluate(element => element.complete && element.naturalWidth > 0)).toBe(true);
    expect(await image.getAttribute("alt")).toBeTruthy();
  }
  const audio = page.locator("audio");
  await expect(audio).toHaveCount(11);
  for (const clip of await audio.all()) {
    await clip.evaluate(element => { element.preload = "metadata"; element.load(); });
    await expect.poll(() => clip.evaluate(element => Number.isFinite(element.duration) && element.duration > 0)).toBe(true);
    await expect(clip).toHaveAttribute("controls", "");
  }
  // Exercise browser decoding/playback once, without altering captured source media.
  await audio.first().evaluate(element => element.play());
  await expect.poll(() => audio.first().evaluate(element => element.currentTime)).toBeGreaterThan(0);
  await audio.first().evaluate(element => element.pause());
  const video = page.locator("video");
  await expect(video).toHaveCount(1);
  await expect.poll(() => video.evaluate(element => Number.isFinite(element.duration) && element.duration > 0)).toBe(true);
  await video.evaluate(element => element.play());
  await expect.poll(() => video.evaluate(element => element.currentTime)).toBeGreaterThan(0);
  await video.evaluate(element => element.pause());
  for (const name of ["ConduitOS", "Browser", "Conversational"]) {
    await page.getByRole("button", { name, exact: true }).click();
    await expect(page.locator(".body-panel:visible")).toHaveCount(13);
  }
  await page.getByRole("button", { name: "Compare all three", exact: true }).click();
  await expect(page.locator(".body-panel:visible")).toHaveCount(39);
  await page.locator("#moment-3").screenshot({ path: join(output, "desktop-wake.png") });
  await page.locator("#moment-8").screenshot({ path: join(output, "desktop-fault.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.locator("#moment-8").screenshot({ path: join(output, "mobile-fault.png") });
  expect(failures).toEqual([]);
  await writeFile(join(output, "review.json"), JSON.stringify({
    schema: "conduit.evidence/documentary-browser-review@1", browser: await browser.version(),
    retries: 0, workers: 1, images: 37, audio_clips: 11, video_clips: 1,
    default_json_collapsed: true, body_navigation: true, mobile_overflow: false,
    boundary: "Browser media decoding and presentation checks, not physical speaker or human listening acceptance.",
  }, null, 2), { flag: "wx" });
  console.log(`DOCUMENTARY BROWSER CHECK COMPLETE: ${output}`);
} finally {
  await browser?.close();
  entrance.child.kill();
}
