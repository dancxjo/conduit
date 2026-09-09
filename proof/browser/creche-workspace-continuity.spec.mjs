import { expect, test } from "@playwright/test";
import { mkdtemp, symlink, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { startStaticProduct } from "./tour-test-server.mjs";

let root, entrance;
test.beforeAll(async () => {
  root = await mkdtemp(join(tmpdir(), "conduit-workspace-continuity-"));
  await symlink(resolve("target/workspace-product"), join(root, "workspace"));
  await symlink(resolve("target/creche-product"), join(root, "creche"));
  await symlink(resolve("target/tour-product"), join(root, "tour"));
});
test.afterAll(async () => { if (root) await rm(root, { recursive: true }); });
test.beforeEach(async () => { entrance = await startStaticProduct(root, "/conduit/"); });
test.afterEach(() => entrance?.child.kill());

test("Crèche respects the live Body owner and returns a retained Body to its Forms", async ({ page, context }) => {
  await page.goto(new URL("workspace/", entrance.url).href);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const first = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  const creche = await context.newPage();
  await creche.goto(new URL("creche/", entrance.url).href);
  await expect(creche.locator("#workspace")).toContainText("This Body is open in another window");
  await expect(creche.getByRole("button", { name: "Birth Body", exact: true })).toHaveCount(0);
  await page.close();
  await creche.reload();
  await expect(creche.getByRole("heading", { name: "Return to your Body", exact: true })).toBeVisible();
  await creche.getByRole("link", { name: "Open your Body", exact: true }).click();
  await expect(creche.locator("[data-play-state]")).toHaveText("Playing");
  const returned = await creche.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(returned.body_id).toBe(first.body_id);
  expect(returned.host_id).toBe(first.host_id);
  expect(returned.boot_id).not.toBe(first.boot_id);
  expect(returned.active_play_id).not.toBe(first.active_play_id);
  await creche.keyboard.press("r");
  await expect(creche.locator("[data-form-output] output:visible")).toHaveText("r");
});


test('the actual Gallery Use link returns to the same Body with the selected Form listening', async ({ page }) => {
  await page.goto(new URL('workspace/', entrance.url).href);
  await page.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  const original = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.goto(new URL('tour/', entrance.url).href);
  await page.getByRole('button', { name: 'Form Gallery', exact: true }).click();
  const entry = page.locator('[data-application-key="gallery-cards"] > [data-application-component="panel"]')
    .filter({ has: page.getByRole('heading', { name: 'Desk Telegraph', exact: true }) });
  await entry.getByRole('link', { name: 'Use in your Body', exact: true }).click();
  await expect(page.locator('#surface-title')).toHaveText('Desk Telegraph');
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  const arrived = await page.evaluate(() => globalThis.__conduitWorkspace.current());
  expect(arrived.body_id).toBe(original.body_id);
  expect(arrived.initial_forms).toHaveLength(2);
  expect(new URL(page.url()).pathname).toBe(new URL('workspace/', entrance.url).pathname);
  expect(new URL(page.url()).search).toBe('');
  await page.keyboard.type('home'); await page.keyboard.press('Enter');
  await expect(page.locator('[data-form-output] output:visible')).toHaveText('home');
});
