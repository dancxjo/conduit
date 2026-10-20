import { writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { startStaticProduct } from '/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/stage04-refresh/proof/browser/static-product-server.mjs';
import { classifyRequestFailures, drainRequestFailures } from '/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/native-panels/tools/ci/pipeline/targets/browser-network-evidence.mjs';

const require = createRequire('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/stage04-refresh/proof/browser/package.json');
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
  const failedRequests = [];
  page.on('pageerror', error => errors.push(error.message));
  const recordRequestFailure = request => failedRequests.push((async () => {
    const response = await request.response();
    return { url: request.url(), errorText: request.failure()?.errorText,
      method: request.method(), resourceType: request.resourceType(),
      ...request.timing(), status: response?.status(),
      contentLength: Number(response?.headers()['content-length']) };
  })());
  page.on('requestfailed', recordRequestFailure);
  await page.goto(entrance.url);
  await page.waitForFunction(() => document.documentElement.dataset.conduitLoadState === 'ready');
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
  const requestFailures = await drainRequestFailures(failedRequests);
  // End the measured startup/audit window before taking its final snapshot;
  // teardown notifications are not runtime observations of this scenario.
  page.off('requestfailed', recordRequestFailure);
  const timing = await page.evaluate(() => ({ timeOrigin: performance.timeOrigin,
    resources: performance.getEntriesByType('resource').map(resource => resource.toJSON()) }));
  const network = classifyRequestFailures(requestFailures, timing);
  errors.push(...network.errors);
  writeFileSync(output, JSON.stringify({ violations: axe.violations, pageErrors: errors, checkedLinks,
    requestFailures, resourceTimings: timing,
    completedAbortNotifications: network.completedAbortNotifications }, null, 2));
  if (axe.violations.length || errors.length) {
    // Failed lanes do not publish their product artifact. Preserve the exact
    // classifier inputs in the retained job log rather than only its verdict.
    const failedUrls = new Set(requestFailures.map(request => request.url));
    const networkEvidence = { requestFailures, timeOrigin: timing.timeOrigin,
      resources: timing.resources.filter(resource => failedUrls.has(resource.name)) };
    throw new Error(`Workspace accessibility/runtime failure: ${JSON.stringify({
      violations: axe.violations, errors, networkEvidence,
    })}`);
  }
} finally {
  await browser?.close();
  entrance.child.kill();
}
