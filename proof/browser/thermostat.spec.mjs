import { expect, test } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";

async function screenshot(page, name, info) {
  const directory = process.env.CONDUIT_THERMOSTAT_SCREENSHOT_DIR;
  if (directory) await mkdir(directory, { recursive: true });
  await page.screenshot({ path: directory ? join(directory, name) : info.outputPath(name), fullPage: true, animations: "disabled" });
}
async function clickAction(page, control) {
  const response = page.waitForResponse((r) => new URL(r.url()).pathname === "/action" && r.request().method() === "POST");
  await control.click();
  expect((await response).status()).toBe(200);
  await expect(page.locator("main")).toHaveAttribute("aria-busy", "false");
}

test("thermostat semantic controls, revision guard, bounds and responsive encounter", async ({ page, request }, info) => {
  const initialFace = await (await request.get("/face")).json();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Living room" })).toBeVisible();
  await expect(page.locator("#target")).toHaveText("21.0");
  await expect(page.locator("#current")).toHaveText("Sensor unavailable");
  await expect(page.locator("#status")).toHaveText("Control is off");
  await expect(page.getByText(/No temperature sensor or heating, cooling, or fan hardware is connected/)).toBeVisible();
  await screenshot(page, "thermostat-desktop.png", info);

  const lower = page.getByRole("button", { name: "Lower target by 0.5°C", exact: true });
  const raise = page.getByRole("button", { name: "Raise target by 0.5°C", exact: true });
  await clickAction(page, raise);
  await expect(page.locator("#target")).toHaveText("21.5");
  await clickAction(page, lower);
  await expect(page.locator("#target")).toHaveText("21.0");

  const modes = page.getByRole("group", { name: "Climate mode", exact: true });
  for (const name of ["Heat", "Cool", "Auto", "Off"]) {
    const button = modes.getByRole("button", { name, exact: true });
    await clickAction(page, button);
    await expect(button).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#status")).toHaveText(name === "Off" ? "Control is off" : "Waiting for a temperature sensor");
    await expect(modes.locator('[aria-pressed="true"]')).toHaveCount(1);
  }
  const fan = page.getByRole("group", { name: "Fan setting", exact: true });
  for (const name of ["On", "Auto"]) {
    const button = fan.getByRole("button", { name, exact: true });
    await clickAction(page, button);
    await expect(button).toHaveAttribute("aria-pressed", "true");
  }
  const presets = page.getByRole("group", { name: "Temperature presets", exact: true });
  for (const [name, temperature] of [["Eco", "18.0"], ["Sleep", "19.0"], ["Comfort", "21.0"]]) {
    const button = presets.getByRole("button", { name, exact: true });
    await clickAction(page, button);
    await expect(button).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#target")).toHaveText(temperature);
  }
  await clickAction(page, modes.getByRole("button", { name: "Cool", exact: true }));
  await expect(page.locator("#target")).toHaveText("24.0");
  for (const [name, temperature] of [["Eco", "26.0"], ["Sleep", "25.0"], ["Comfort", "24.0"]]) {
    await clickAction(page, presets.getByRole("button", { name, exact: true }));
    await expect(page.locator("#target")).toHaveText(temperature);
  }
  await clickAction(page, modes.getByRole("button", { name: "Off", exact: true }));
  await expect(page.locator("#target")).toHaveText("21.0");

  // Native keyboard activation follows the same admitted action route.
  await raise.focus();
  const keyResponse = page.waitForResponse((r) => new URL(r.url()).pathname === "/action");
  await page.keyboard.press("Enter");
  expect((await keyResponse).status()).toBe(200);
  await expect(page.locator("#target")).toHaveText("21.5");
  await clickAction(page, modes.getByRole("button", { name: "Cool", exact: true }));
  await expect(page.locator("#target")).toHaveText("21.5");
  await clickAction(page, modes.getByRole("button", { name: "Off", exact: true }));
  await expect(page.locator("#target")).toHaveText("21.5");
  const beforeReload = await (await request.get("/face")).json();
  await page.reload();
  await expect(page.locator("#target")).toHaveText("21.5");
  expect(await (await request.get("/face")).json()).toEqual(beforeReload);

  await clickAction(page, raise);
  const current = await (await request.get("/face")).json();
  const stale = await request.post("/action", { data: { revision: beforeReload.revision, action_id: "thermostat.lower" } });
  expect(stale.status()).toBe(409);
  expect((await stale.json()).error).toBeTruthy();
  expect(await (await request.get("/face")).json()).toEqual(current);
  await expect(page.locator("#target")).toHaveText("22.0");

  for (let target = 225; target <= 300; target += 5) {
    await clickAction(page, raise);
    await expect(page.locator("#target")).toHaveText((target / 10).toFixed(1));
  }
  await expect(raise).toBeDisabled();
  await expect(lower).toBeEnabled();
  for (let target = 295; target >= 100; target -= 5) {
    await clickAction(page, lower);
    await expect(page.locator("#target")).toHaveText((target / 10).toFixed(1));
  }
  await expect(lower).toBeDisabled();
  await expect(raise).toBeEnabled();
  await clickAction(page, presets.getByRole("button", { name: "Comfort", exact: true }));
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator("#target")).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await screenshot(page, "thermostat-mobile.png", info);
  const finalFace = await (await request.get("/face")).json();
  expect(finalFace.body_id).toBe(initialFace.body_id);
  expect(finalFace.basis).toEqual(initialFace.basis);
  expect(errors).toEqual([]);
});
