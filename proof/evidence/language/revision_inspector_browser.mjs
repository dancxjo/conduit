// Browser interaction proof for the recorded inspector; does not run a parser.
import assert from 'node:assert/strict';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
const [html, trace, output] = process.argv.slice(2);
if (!html || !trace || !output) throw Error('Usage: node revision_inspector_browser.mjs inspector.html actual.jsonl output-directory');
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const records = readFileSync(trace, 'utf8').split('\n').filter(line => line.trim()).map(JSON.parse);
assert(records.length > 1);
mkdirSync(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(pathToFileURL(resolve(html)).href);
  assert.equal(await page.locator('#ordinal').innerText(), `1 / ${records.length}`);
  assert(await page.locator('#previous').isDisabled());
  await page.locator('#next').click();
  assert.equal(await page.locator('#ordinal').innerText(), `2 / ${records.length}`);
  await page.locator('#previous').click();
  assert.equal(await page.locator('#ordinal').innerText(), `1 / ${records.length}`);
  await page.locator('#event').fill(String(records.length - 1));
  await page.locator('#event').dispatchEvent('input');
  assert(await page.locator('#next').isDisabled());
  assert.equal(await page.locator('#candidates tr').count(), records.at(-1).candidates?.length ?? 0);
  assert.deepEqual(JSON.parse(await page.locator('#raw').textContent()), records.at(-1));
  await page.screenshot({ path: join(output, 'recorded-snapshot.png'), fullPage: false });
  await page.locator('#file').setInputFiles(resolve(trace));
  await page.waitForFunction(() => document.getElementById('ordinal').textContent.startsWith('1 / '));
  assert.equal(await page.locator('#error').innerText(), '');
  assert.deepEqual(JSON.parse(await page.locator('#raw').textContent()), records[0]);
  await page.locator('#file').setInputFiles({ name: 'malformed.jsonl', mimeType: 'application/json', buffer: Buffer.from('{malformed}\n') });
  await page.waitForFunction(() => document.getElementById('error').textContent.length > 0);
  assert.deepEqual(JSON.parse(await page.locator('#raw').textContent()), records[0]);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: join(output, 'recorded-mobile.png'), fullPage: false });
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
  assert.equal(overflow, false, 'Mobile page must not overflow horizontally');
  assert.deepEqual(errors, []);
  const result = { recorded_events: records.length, native_event_equality: true, navigation: true, file_load: true, malformed_load_preserves_previous: true, mobile_horizontal_overflow: false, browser_errors: errors, live_file_follow_verified: false, parser_executed_by_viewer: false };
  writeFileSync(join(output, 'browser-proof.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result));
} finally {
  await browser.close();
}
