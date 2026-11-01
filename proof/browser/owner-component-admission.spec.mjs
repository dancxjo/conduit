import { expect, test } from '@playwright/test';

// The public SDK must not install an optional Back before its byte/Host gate.
// This exercises real module evaluation, not a mocked element registry.
test('loading the public SDK leaves the optional owner component unregistered', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  expect(await page.evaluate(() => Boolean(customElements.get('conduit-owner-face')))).toBe(false);
  await page.evaluate(() => import('/targets/browser/sdk/browser-sdk.mjs'));
  expect(await page.evaluate(() => Boolean(customElements.get('conduit-owner-face')))).toBe(false);
});
