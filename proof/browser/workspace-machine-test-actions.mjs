import { expect } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

export function startWorkspaceMachineProduct() {
  return startStaticProduct("target/workspace-product", "/conduit/workspace/");
}

export async function openWorkspaceMachineRunner(page, entrance) {
  await page.goto(entrance.url);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "parts / hosts", exact: true }).click();
  await page.getByRole("button", { name: "Add a host", exact: true }).click();
  await page.getByRole("button", { name: "Another machine", exact: true }).click();
  await page.getByRole("button", { name: "Continue with machine catalog", exact: true }).click();
  const runner = page.locator(".physical-host-runner");
  await expect(runner).toBeVisible();
  return runner;
}
