// Opt-in retained-source fault injection in a fresh, isolated browser context.
import { chromium, expect } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const receiptPath = process.argv[2];
if (!receiptPath) throw new Error('an explicit receipt output path is required');
const hash = value => 'sha256:' + createHash('sha256').update(value).digest('hex');
const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext();
  const page = await context.newPage(), errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('https://dancxjo.github.io/conduit/handbook/', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForFunction(() => !!globalThis.__conduitApplication, null, { timeout: 30000 });
  const original = await page.evaluate(() => globalThis.__conduitApplication.snapshot());
  const packageIdentity = await page.evaluate(() => globalThis.__conduitBrowserApplication.manifest.packageDigest);
  await page.getByRole('button', { name: 'Lull', exact: true }).click();
  const retained = await page.evaluate(async () => {
    const store = globalThis.__conduitBrowserApplication.storage;
    const record = await store.readJson('body-continuity');
    const inventory = JSON.parse(record.source);
    const lesson = inventory.plots.find(plot => plot.entry === 'convert-pitch-demo');
    if (!lesson) throw new Error('missing retained pitch lesson');
    const canonical = lesson.source;
    lesson.source = canonical.replace('source = 1kHz', 'source = "1kHz"');
    if (lesson.source === canonical) throw new Error('fault injection did not change source');
    const changed = { ...record, source: JSON.stringify(inventory) };
    await store.writeJson('body-continuity', changed);
    return { record: changed, canonical, malformed: lesson.source };
  });
  await page.locator('[data-handbook-application]').getByText('Your body and browser', { exact: true }).click();
  await page.getByRole('button', { name: 'Release this tab', exact: true }).click();
  await page.reload();
  await expect(page.locator('[data-session-status]')).toContainText('Retained Body recovery refused', { timeout: 30000 });
  await expect(page.locator('[data-session-status]')).toContainText('CND-FRM-051');
  await page.waitForFunction(() => !!globalThis.__conduitBrowserApplication, null, { timeout: 30000 });
  const editor = page.getByRole('textbox', { name: 'Plot source', exact: true });
  const chooser = page.getByLabel('Choose an example', { exact: true });
  await expect(editor).toBeEnabled(); await expect(chooser).toBeEnabled();
  await expect(editor).toHaveValue(retained.malformed);
  const retainedChoice = await chooser.inputValue(), retainedLabel = await chooser.locator('option:checked').textContent();
  expect(retainedLabel).toContain('(retained)');
  await chooser.selectOption({ label: retainedLabel.replace('(retained)', '(packaged)') });
  await expect(editor).toHaveValue(retained.canonical);
  await chooser.selectOption(retainedChoice); await expect(editor).toHaveValue(retained.malformed);
  const continuity = () => page.evaluate(() => globalThis.__conduitBrowserApplication.storage.readJson('body-continuity'));
  expect(await continuity()).toEqual(retained.record);
  await editor.fill(retained.canonical);
  await page.getByRole('button', { name: 'Try in my Handbook', exact: true }).click();
  await expect(page.locator('[data-check]')).toContainText('Source checks. The retained Body has not been changed', { timeout: 30000 });
  expect(await continuity()).toEqual(retained.record);
  const downloadEvent = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Download original retained Body record', exact: true }).click();
  const download = await downloadEvent, downloaded = await readFile(await download.path(), 'utf8');
  expect(downloaded).toBe(JSON.stringify(retained.record, null, 2));
  const consent = page.getByRole('checkbox', { name: 'I have saved what I need', exact: false });
  const replacement = page.getByRole('button', { name: 'Start a new Handbook Body', exact: true });
  await expect(replacement).toBeDisabled(); await consent.check(); await expect(replacement).toBeEnabled();
  await consent.uncheck(); await expect(replacement).toBeDisabled();
  expect(await continuity()).toEqual(retained.record);
  expect(await page.evaluate(() => !!globalThis.__conduitApplication)).toBe(false); expect(errors).toEqual([]);
  const report = { observed_at_utc: new Date().toISOString(), url: page.url(), browser: browser.version(),
    fresh_context: true, method: 'One isolated Chromium context; native click/selectOption/fill/check. Only its own retained continuity source is fault-injected; public package unmodified. Initial probe stopped on an ambiguous disclosure selector; corrected scoped selector before this verification run.',
    package_digest: packageIdentity, original_body_id: original.bodyId, retained_record_sha256: hash(JSON.stringify(retained.record)),
    canonical_source_sha256: hash(retained.canonical), malformed_source: retained.malformed,
    startup_diagnostic: await page.locator('[data-session-status]').textContent(), editor_enabled: true, selector_enabled: true,
    original_and_packaged_sources_independently_inspected: true, correction_checked_without_continuity_mutation: true,
    backup_filename: download.suggestedFilename(), backup_sha256: hash(downloaded), backup_exact_original_record: true,
    replacement_consent_guard_verified: true, no_replacement_started: true, page_errors: errors,
    limits: ['Synthetic retained-source corruption in an isolated context, not the historical reporting browser.',
      'Does not prove semantic migration of retained identities, separately archived history backup, unpromoted live changes or release-train acceptance.'] };
  await writeFile(receiptPath, JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ package: packageIdentity, backupExact: true, continuityUnchanged: true, diagnostic: 'CND-FRM-051' }));
} finally { await browser.close(); }
