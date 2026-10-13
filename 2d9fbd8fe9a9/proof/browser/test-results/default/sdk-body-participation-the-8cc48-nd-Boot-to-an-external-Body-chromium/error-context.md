# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: sdk-body-participation.spec.mjs >> the browser SDK lends its exact admitted Host and Boot to an external Body
- Location: sdk-body-participation.spec.mjs:10:1

# Error details

```
Error: expect(locator).toHaveText(expected) failed

Locator: locator('#status')
Expected: "Browser Host participating"
Timeout: 15000ms
Error: element(s) not found

Call log:
  - Expect "toHaveText" with timeout 15000ms
  - waiting for locator('#status')

```

# Test source

```ts
  1  | import { expect, test } from "@playwright/test";
  2  | import { prepareFieldStationPackage } from "./field-station-package.mjs";
  3  | import { startPresenceProbe } from "./browser-presence-support.mjs";
  4  | 
  5  | test.beforeAll(async ({}, testInfo) => {
  6  |   testInfo.setTimeout(120_000);
  7  |   await prepareFieldStationPackage();
  8  | });
  9  | 
  10 | test("the browser SDK lends its exact admitted Host and Boot to an external Body", async ({ page }) => {
  11 |   const probe = await startPresenceProbe();
  12 |   try {
  13 |     const pageErrors = [];
  14 |     page.on("pageerror", (error) => pageErrors.push(error.message));
  15 |     await page.goto(`/proof/browser/sdk-body-participation/?body=${encodeURIComponent(probe.url)}`);
> 16 |     await expect(page.locator("#status")).toHaveText("Browser Host participating", { timeout: 15_000 });
     |                                           ^ Error: expect(locator).toHaveText(expected) failed
  17 |     await expect.poll(() => page.evaluate(() =>
  18 |       globalThis.__conduitSdkParticipation?.participation.presenceState())).toBe("available");
  19 |     await expect.poll(() => page.evaluate(() => {
  20 |       const evidence = globalThis.__conduitSdkParticipation?.evidence();
  21 |       return Boolean(evidence?.biography && evidence?.offer);
  22 |     })).toBe(true);
  23 | 
  24 |     const truth = await page.evaluate(() => {
  25 |       const { host, participation, evidence } = globalThis.__conduitSdkParticipation;
  26 |       const credential = participation.membershipCredential();
  27 |       const observed = evidence();
  28 |       const currentPart = observed.biography.membership.parts
  29 |         .find(({ part_id }) => part_id === credential.part_id);
  30 |       return {
  31 |         host: host.current(),
  32 |         participant: {
  33 |           hostId: participation.hostId,
  34 |           bootId: participation.bootId,
  35 |           bodyId: credential.body_id,
  36 |         },
  37 |         credential: {
  38 |           hostId: credential.host_id,
  39 |           bootId: credential.boot_id,
  40 |           bodyId: credential.body_id,
  41 |         },
  42 |         biography: {
  43 |           bodyId: observed.biography.body_id,
  44 |           hostId: currentPart.current.host_id,
  45 |           bootId: currentPart.current.boot_id,
  46 |         },
  47 |         offer: {
  48 |           hostId: observed.offer.host_id,
  49 |           bootId: observed.offer.boot_id,
  50 |         },
  51 |       };
  52 |     });
  53 |     expect(truth.participant).toEqual({
  54 |       hostId: truth.host.id,
  55 |       bootId: truth.host.bootId,
  56 |       bodyId: truth.credential.bodyId,
  57 |     });
  58 |     expect(truth.credential).toEqual(truth.participant);
  59 |     expect(truth.biography).toEqual(truth.participant);
  60 |     expect(truth.offer).toEqual({ hostId: truth.host.id, bootId: truth.host.bootId });
  61 |     expect(pageErrors).toEqual([]);
  62 | 
  63 |     const finalSequence = await page.evaluate(() =>
  64 |       globalThis.__conduitSdkParticipation.participation.close());
  65 |     await expect.poll(probe.output).toContain(`unavailable reason=session-lost sequence=${finalSequence}`);
  66 |     await expect.poll(() => probe.process.exitCode).toBe(0);
  67 |   } finally {
  68 |     if (probe.process.exitCode === null) probe.process.kill("SIGTERM");
  69 |   }
  70 | });
  71 | 
```