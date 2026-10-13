// Explicit historical DOM reconstruction; no runtime Show or input authority.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
const runtime = process.env.CONDUIT_BROWSER_RUNTIME_ROOT;
const recordsPath = process.env.CONDUIT_PARSEC_BROWSER_RECORDS;
assert.ok(runtime && path.isAbsolute(runtime));
assert.ok(recordsPath && path.isAbsolute(recordsPath));
const require = createRequire(path.join(runtime, 'package.json'));
const { chromium } = require('@playwright/test');
assert.equal(require('@playwright/test/package.json').version, '1.62.0');
const capture = JSON.parse(await readFile(recordsPath, 'utf8'));
assert.equal(capture.schema, 'conduit.proof/parsec-component-reconstruction@1');
assert.equal(capture.records.length, 4);
const module = await readFile(new URL('../../targets/browser/handbook/instruction-component.mjs', import.meta.url), 'utf8');
const owner = await readFile(new URL('../../targets/browser/handbook/owner-face-component.mjs', import.meta.url), 'utf8');
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  await page.route('http://component.test/**', async route => {
    const name = new URL(route.request().url()).pathname;
    await route.fulfill({ contentType: name.endsWith('.mjs') ? 'text/javascript' : 'text/html',
      body: name === '/instruction.mjs' ? module : name === '/owner.mjs' ? owner : '<!doctype html><html lang="en"><title>Captured component</title><body></body></html>' });
  });
  await page.goto('http://component.test/');
  await page.evaluate(async () => {
    await import('/instruction.mjs'); await import('/owner.mjs');
    document.body.append(document.createElement('conduit-owner-face'), document.createElement('conduit-instruction-component'));
  });
  for (const record of capture.records) {
    await page.evaluate(record => {
      const owner = document.querySelector('conduit-owner-face');
      const face = record.face;
      const actions = face.actions.map(action => ({ ...action, availability: 'available', arguments: [] }));
      const view = { face_id: face.identity, face_revision: record.face_revision_decimal,
        show_state: 'unavailable', interactions_admitted: false,
        subjects: face.subjects.map(subject => ({ ...subject, text: [], properties: [], values: [] })), relationships: [], actions };
      owner.show(view);
      for (const details of owner.shadowRoot.querySelectorAll('details')) details.open = true;
      globalThis.originalFaceMarkup = owner.shadowRoot.innerHTML;
      document.querySelector('conduit-instruction-component').showReconstruction(record);
    }, record);
    assert.equal(await page.getByRole('note').textContent(), record.wording);
    assert.equal(await page.getByRole('note').getAttribute('aria-label'), 'Input instruction from captured execution');
    assert.equal(await page.locator('conduit-owner-face').getByRole('button').count(), 1);
    assert.equal(await page.locator('conduit-owner-face').getByRole('button').textContent(), record.face.actions[0].name);
    assert.match(record.face_revision_decimal, /^[0-9]+$/u);
    assert.match(record.mapping_revision_decimal, /^[0-9]+$/u);
    assert.equal(await page.locator('conduit-owner-face').getByRole('button').isDisabled(), true);
    assert.equal(await page.locator('conduit-instruction-component').getByRole('button').count(), 0);
    await page.evaluate(() => document.querySelector('conduit-instruction-component').withdraw());
    assert.equal(await page.getByRole('note').count(), 0);
    assert.equal(await page.evaluate(() => document.querySelector('conduit-owner-face').shadowRoot.innerHTML === globalThis.originalFaceMarkup), true);
  }
  const refused = await page.evaluate(() => {
    const element = document.querySelector('conduit-instruction-component');
    return [null, { wording: 'invented' }, { wording: 'x'.repeat(257), provenance: { action: 'a', source_show: 's' } }]
      .every(record => { try { element.showReconstruction(record); return false; } catch { return element.hidden; } });
  });
  assert.equal(refused, true);
  console.log('PASS: four captured instruction DOM reconstructions; withdrawal and bounded refusal; no input authority');
} finally { await browser.close(); }
