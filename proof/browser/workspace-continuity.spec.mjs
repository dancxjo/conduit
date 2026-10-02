import { expect, test } from "@playwright/test";
import { mkdtemp, symlink, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { startStaticProduct } from "./static-product-server.mjs";

let root, entrance;
test.beforeAll(async () => {
  root = await mkdtemp(join(tmpdir(), "conduit-workspace-continuity-"));
  await symlink(resolve("target/workspace-product"), join(root, "workspace"));
});
test.afterAll(async () => { if (root) await rm(root, { recursive: true }); });
test.beforeEach(async () => { entrance = await startStaticProduct(root, "/conduit/"); });
test.afterEach(() => entrance?.child.kill());

test("Workspace returns a retained Body to its Plots without a parallel application", async ({ page, context }) => {
  await page.goto(new URL("workspace/", entrance.url).href);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const first = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  const returnedWorkspace = await context.newPage();
  await returnedWorkspace.goto(new URL("workspace/", entrance.url).href);
  await page.close();
  await returnedWorkspace.reload();
  await expect(returnedWorkspace.locator("[data-play-state]")).toHaveText("Playing");
  const returned = await returnedWorkspace.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(returned.body_id).toBe(first.body_id);
  expect(returned.host_id).toBe(first.host_id);
  expect(returned.boot_id).not.toBe(first.boot_id);
  expect(returned.active_play_id).not.toBe(first.active_play_id);
  await returnedWorkspace.keyboard.press("r");
  await expect(returnedWorkspace.locator("[data-plot-output] output:visible")).toHaveText("r");
});
