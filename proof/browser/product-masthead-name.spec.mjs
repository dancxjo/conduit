import { test, expect } from '@playwright/test';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { PRODUCT_DESTINATIONS } from '../../semantics/face/assets/product-masthead.mjs';

test('generated masthead exposes the declared destination names', async ({ page }) => {
  const directory = await mkdtemp(join(tmpdir(), 'conduit-masthead-name-'));
  try {
    const input = join(directory, 'input.html'), output = join(directory, 'output.html');
    await writeFile(input, '<!doctype html><html><body><!-- conduit-product-masthead --></body></html>');
    execFileSync(process.execPath, ['targets/browser/tools/render-product-masthead.mjs', input, output, 'home', 'Exact <status>']);
    await page.setContent(await readFile(output, 'utf8'));
    for (const destination of PRODUCT_DESTINATIONS) {
      await expect(page.getByRole('link', { name: destination.label, exact: true })).toHaveText(destination.label);
    }
    const home = page.getByRole('link', { name: PRODUCT_DESTINATIONS[0].label, exact: true });
    await expect(home).toHaveAttribute('href', '/conduit');
    await expect(home).toHaveAttribute('aria-current', 'page');
    await page.keyboard.press('Tab');
    await expect(home).toBeFocused();
    await expect(page.getByRole('status')).toHaveText('Exact <status>');
  } finally { await rm(directory, { recursive: true, force: true }); }
});
