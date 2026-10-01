import { expect, test } from "@playwright/test";
import { prepareFieldStationPackage } from "./field-station-package.mjs";
import { startPresenceProbe } from "./browser-presence-support.mjs";

test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(120_000);
  await prepareFieldStationPackage();
});

test("the browser SDK lends its exact admitted Host and Boot to an external Body", async ({ page }) => {
  const probe = await startPresenceProbe();
  try {
    const pageErrors = [];
    page.on("pageerror", (error) => pageErrors.push(error.message));
    await page.goto(`/proof/browser/sdk-body-participation/?body=${encodeURIComponent(probe.url)}`);
    await expect(page.locator("#status")).toHaveText("Browser Host participating", { timeout: 15_000 });
    await expect.poll(() => page.evaluate(() =>
      globalThis.__conduitSdkParticipation?.participation.presenceState())).toBe("available");
    await expect.poll(() => page.evaluate(() => {
      const evidence = globalThis.__conduitSdkParticipation?.evidence();
      return Boolean(evidence?.biography && evidence?.offer);
    })).toBe(true);

    const truth = await page.evaluate(() => {
      const { host, participation, evidence } = globalThis.__conduitSdkParticipation;
      const credential = participation.membershipCredential();
      const observed = evidence();
      const currentPart = observed.biography.membership.parts
        .find(({ part_id }) => part_id === credential.part_id);
      return {
        host: host.current(),
        participant: {
          hostId: participation.hostId,
          bootId: participation.bootId,
          bodyId: credential.body_id,
        },
        credential: {
          hostId: credential.host_id,
          bootId: credential.boot_id,
          bodyId: credential.body_id,
        },
        biography: {
          bodyId: observed.biography.body_id,
          hostId: currentPart.current.host_id,
          bootId: currentPart.current.boot_id,
        },
        offer: {
          hostId: observed.offer.host_id,
          bootId: observed.offer.boot_id,
        },
      };
    });
    expect(truth.participant).toEqual({
      hostId: truth.host.id,
      bootId: truth.host.bootId,
      bodyId: truth.credential.bodyId,
    });
    expect(truth.credential).toEqual(truth.participant);
    expect(truth.biography).toEqual(truth.participant);
    expect(truth.offer).toEqual({ hostId: truth.host.id, bootId: truth.host.bootId });
    expect(pageErrors).toEqual([]);

    const finalSequence = await page.evaluate(() =>
      globalThis.__conduitSdkParticipation.participation.close());
    await expect.poll(probe.output).toContain(`left sequence=${finalSequence}`);
    await expect.poll(() => probe.process.exitCode).toBe(0);
  } finally {
    if (probe.process.exitCode === null) probe.process.kill("SIGTERM");
  }
});
