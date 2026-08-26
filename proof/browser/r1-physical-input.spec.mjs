import { expect, test } from "@playwright/test";

test("two Chromium peers deliver exact physical-session inputs", async ({ browser }) => {
  const line = process.env.CONDUIT_R1_INPUT_LINE;
  expect(line).toMatch(/^ws:\/\/127\.0\.0\.1:\d+$/);
  const context = await browser.newContext();
  const pageA = await context.newPage();
  const pageB = await context.newPage();
  const pageUrl = (peer) => `/proof/browser/r1-three-peer-input.test.html?peer=${peer}&line=${encodeURIComponent(line)}`;

  await pageA.goto(pageUrl("browser-a"));
  await expect(pageA.getByRole("status")).toHaveText("ready");
  await pageB.goto(pageUrl("browser-b"));
  await expect(pageB.getByRole("status")).toHaveText("ready");

  for (const page of [pageA, pageB]) {
    await page.getByRole("button", { name: "Hold to control LED" }).focus();
    await page.keyboard.down("Space");
    await page.keyboard.up("Space");
    await expect(page.getByRole("status")).toHaveText("complete");
  }
  const proofA = await pageA.evaluate(() => globalThis.__r1InputPeer.proof());
  const proofB = await pageB.evaluate(() => globalThis.__r1InputPeer.proof());
  expect(proofA.acknowledgements).toEqual([
    { mergedSequence: 2, level: true },
    { mergedSequence: 3, level: false },
  ]);
  expect(proofB.acknowledgements).toEqual([
    { mergedSequence: 4, level: true },
    { mergedSequence: 5, level: false },
  ]);
  console.log(JSON.stringify({
    schema: "conduit.r1/physical-browser-input@1",
    browserA: proofA,
    browserB: proofB,
  }));
  await context.close();
});
