import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;
test.beforeEach(async () => { entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
test.afterEach(() => entrance?.child.kill());

test("a second distinct browser Host explicitly joins through one canonical Body invitation", async ({ page, context }) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();

  await expect(page.getByRole("button", { name: "Parts / Hosts", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Parts / Hosts", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Parts and Hosts", exact: true })).toBeVisible();
  await expect(page.locator(".member-card")).toHaveCount(1);
  await expect(page.locator(".member-card")).toContainText("admitted · present · local Host");
  const before = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);

  await page.getByRole("button", { name: "Invite another Host", exact: true }).click();
  await expect(page.getByText("Invitation ready", { exact: true })).toBeVisible();
  const link = await page.locator("[data-invitation-link]").inputValue();
  const afterOffer = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  expect(afterOffer).toEqual(before);
  expect(new URL(link).hash).toContain("body-invitation=");

  const joining = await context.newPage();
  await joining.goto(link);
  await expect(joining.getByText("Body invitation", { exact: true })).toBeVisible();
  await expect(joining.getByText("It grants no Form or effect authority.")).toBeVisible();
  await joining.getByRole("button", { name: "Join this Body", exact: true }).click();

  await expect(joining.locator(".member-card")).toHaveCount(2);
  await expect(page.locator(".member-card")).toHaveCount(2);
  const authority = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  const receiver = await joining.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  expect(authority.revision).toBe(before.revision + 2);
  expect(receiver).toEqual(authority);
  expect(new Set(authority.parts.map(part => part.current.host_id)).size).toBe(2);
  expect(authority.parts.every(part => part.state === "Admitted" && part.current)).toBe(true);
  const authorityOffers = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers);
  const receiverOffers = await joining.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers);
  expect(authorityOffers).toHaveLength(2);
  expect(receiverOffers).toHaveLength(1);
  for (const offer of authorityOffers) {
    const member = authority.parts.find(part => part.current.host_id === offer.host_id);
    expect(offer.boot_id).toBe(member.current.boot_id);
    expect(offer.offer_generation).toBe(member.current.offer_generation);
  }

  await joining.close();
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await page.getByRole("button", { name: "Parts / Hosts", exact: true }).click();
  await expect(page.locator(".member-card")).toHaveCount(2);
  const restored = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  expect(restored.parts.filter(part => part.current)).toHaveLength(1);
  expect(restored.parts.filter(part => !part.current && part.state === "Admitted")).toHaveLength(1);
  const restoredOffers = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers);
  expect(restoredOffers).toHaveLength(1);
  expect(restoredOffers[0].host_id).toBe(restored.parts.find(part => part.current).current.host_id);
});

test("malformed invitation framing is refused without creating a Body", async ({ page }) => {
  await page.goto(`${entrance.url}#body-invitation=not-json`);
  await expect(page.locator("[data-workspace-notice]")).toContainText("Body invitation is malformed");
  expect(await page.evaluate(() => globalThis.__conduitWorkspace)).toBeUndefined();
});
