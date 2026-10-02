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

test("Host admission and refresh do not require or paint an application surface", async ({ page }) => {
  // A real served package document gives this page the package's same-origin
  // environment without starting a demonstration application or injecting state.
  await page.goto("/target/field-station-sdk/browser-sdk-package.json");
  const truth = await page.evaluate(async () => {
    const { Conduit } = await import("/target/field-station-sdk/browser-sdk.mjs");
    const before = document.body.innerHTML;
    const host = await Conduit.browser();
    const admitted = host.current();
    const refreshed = await host.refresh();
    const afterRootless = document.body.innerHTML;
    const rooted = await Conduit.browser({ root: document.body, durable: false });
    await rooted.refresh();
    return { before, afterRootless, afterRooted: document.body.innerHTML,
      admitted, refreshed, rooted: rooted.current() };
  });
  expect(truth.admitted.schema).toBe("conduit.browser/host-snapshot@1");
  expect(truth.admitted.id).toBeTruthy();
  expect(truth.admitted.bootId).toBeTruthy();
  expect(truth.admitted.offers.length).toBeGreaterThan(0);
  expect(truth.refreshed.id).toBe(truth.admitted.id);
  expect(truth.refreshed.bootId).toBe(truth.admitted.bootId);
  expect(truth.rooted.offers.length).toBeGreaterThan(0);
  expect(truth.afterRootless).toBe(truth.before);
  expect(truth.afterRooted).toBe(truth.before);
});

test("a rootless Host refuses Body execution before acquiring a missing application surface", async ({ page }) => {
  await page.goto("/target/field-station-sdk/browser-sdk-package.json");
  const result = await page.evaluate(async () => {
    const { Conduit, ResourceLossError } = await import("/target/field-station-sdk/browser-sdk.mjs");
    const host = await Conduit.browser({ durable: false });
    const response = await fetch("/plots/clock/main.conduit");
    if (!response.ok) throw new Error("canonical Clock Plot is unavailable");
    const checked = await host.plot(await response.text()).check();
    if (!checked.ok || checked.plots.length !== 1) throw new Error("canonical Clock Plot was refused");
    const body = await host.birth({ name: "Surface admission", plots: checked.plots });
    try {
      await body.wake();
      throw new Error("Body execution unexpectedly acquired an absent surface");
    } catch (error) {
      return { typed: error instanceof ResourceLossError, code: error.code,
        operation: error.operation, message: error.message };
    }
  });
  expect(result).toMatchObject({ typed: true, code: "ApplicationSurfaceUnavailable", operation: "Body.wake" });
  expect(result.message).toContain("connected application-owned root");
});
