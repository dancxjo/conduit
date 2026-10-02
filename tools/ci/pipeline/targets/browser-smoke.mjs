import { writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { startStaticProduct } from '../../../../proof/browser/static-product-server.mjs';

const require = createRequire(new URL('../../../../proof/browser/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const AxeBuilder = require('@axe-core/playwright').default;
const [product, output] = process.argv.slice(2);
if (!product || !output) throw new Error('Expected staged Workspace directory and evidence path');
const entrance = await startStaticProduct(product, '/conduit/workspace/');
let browser;
try {
  browser = await chromium.launch();
  const context = await browser.newContext();
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('requestfailed', request => errors.push(`${request.url()}: ${request.failure()?.errorText}`));
  await page.goto(entrance.url);
  await page.getByRole('button', { name: 'Birth Body', exact: true }).waitFor({ state: 'visible' });
  if (await page.getByRole('main').count() !== 1 || await page.getByRole('heading', { level: 1 }).count() < 1) {
    throw new Error('Workspace lacks its main landmark or heading');
  }
  const axe = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']).analyze();
  const links = await page.locator('a[href], script[src], link[href], img[src]').evaluateAll(nodes =>
    [...new Set(nodes.map(node => node.href || node.src))]);
  let checkedLinks = 0;
  for (const link of links) {
    const url = new URL(link, entrance.url);
    // Other website destinations belong to publication; this proves the local product closure.
    if (!url.href.startsWith(entrance.url) || url.hash) continue;
    const response = await page.request.get(url.href);
    if (!response.ok()) throw new Error(`Broken product link: ${url.href}: ${response.status()}`);
    checkedLinks++;
  }
  writeFileSync(output, JSON.stringify({ violations: axe.violations, pageErrors: errors, checkedLinks }, null, 2));
  if (axe.violations.length || errors.length) throw new Error(`Workspace accessibility/runtime failure: ${JSON.stringify({ violations: axe.violations, errors })}`);
} finally {
  await browser?.close();
  entrance.child.kill();
}
