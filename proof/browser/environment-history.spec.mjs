import { test, expect } from '@playwright/test';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

test('synthetic recording Face renders readable bounded history without fabricated failure values', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(pathToFileURL(resolve('target/environment-history-preview/browser.html')).href);
  await expect(page.getByRole('heading', { name: 'Room conditions history', exact: true })).toBeVisible();
  await expect(page.getByText('Synthetic fixture — not a live sensor connection', { exact: true })).toBeVisible();
  await expect(page.locator('svg circle')).toHaveCount(22);
  await expect(page.getByRole('table').getByRole('row')).toHaveCount(13);
  const failure = page.getByRole('table').getByRole('row').filter({ has: page.getByRole('cell', { name: 'checksum', exact: true }) });
  await expect(failure).toHaveCount(1);
  await expect(failure.getByRole('cell', { name: 'unavailable', exact: true })).toHaveCount(2);
  await expect(page.getByText(/missing sequences 7 through 7/)).toBeVisible();
  await page.getByText('Inspect source', { exact: true }).click();
  await expect(page.getByText(/Producer Host host\/synthetic-environment-preview/)).toBeVisible();
  await page.getByText('Inspect source', { exact: true }).click();
  await page.screenshot({ path: resolve('target/environment-history-preview/browser.png'), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByRole('heading', { name: 'Room conditions history', exact: true })).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
  expect(overflow).toBe(false);
  await page.screenshot({ path: resolve('target/environment-history-preview/browser-mobile.png'), fullPage: true });
  expect(errors).toEqual([]);
});
