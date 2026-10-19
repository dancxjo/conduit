// Historical, readonly reconstruction of an actual guest capture. No browser
// Mask Show, native grant, source owner, or input enactment is created here.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const digest = value => createHash('sha256').update(value).digest('hex');
const capacity = 2 * 1024 * 1024;
const nonempty = value => typeof value === 'string' && value.length > 0;

// This validates the adapter envelope, not the provenance of an arbitrary file.
// The owning xtask must generate it from its validated retained guest records.
export function validateCapture(capture) {
  assert.equal(capture?.schema, 'conduit.proof/native-face-browser-reconstruction@1');
  assert.ok(capture.source && capture.image, 'Source and normal guest image provenance required');
  assert.ok(Array.isArray(capture.records) && capture.records.length > 0 && capture.records.length <= 256);
  return capture.records.map(record => {
    assert.ok(nonempty(record.face_json) && Buffer.byteLength(record.face_json) <= capacity);
    assert.equal(record.face_sha256, digest(record.face_json));
    const face = JSON.parse(record.face_json);
    // Refuse lossy JSON numbers or a serialization rewritten by the adapter.
    assert.equal(JSON.stringify(face), record.face_json, 'Face must survive browser JSON without changed bytes');
    assert.ok(nonempty(face.identity));
    assert.ok(Number.isSafeInteger(face.revision) && face.revision >= 0, 'Unsafe revision is unsupported');
    assert.equal(record.face_id, face.identity);
    assert.equal(record.face_revision, String(face.revision));
    assert.equal(record.native_show?.face_id, face.identity);
    assert.equal(String(record.native_show?.face_revision), record.face_revision);
    assert.ok(nonempty(record.native_show?.show_id));
    assert.equal(record.linear?.face_id, face.identity);
    assert.equal(String(record.linear?.face_revision), record.face_revision);
    assert.equal(record.linear?.proof_class, 'deterministic-semantic-reconstruction');
    assert.equal(record.linear?.input_enactment, false);
    assert.equal(record.linear?.acknowledged_show, null);
    assert.ok(Array.isArray(record.linear?.lines) && record.linear.lines.length > 0);
    const bindings = record.control_bindings;
    assert.equal(bindings?.face_identity, face.identity);
    assert.equal(String(bindings?.face_revision), record.face_revision);
    assert.ok(Array.isArray(bindings?.text_entries) && bindings.text_entries.length <= 64);
    assert.ok(Array.isArray(bindings?.collections) && bindings.collections.length <= 64);
    assert.ok(Array.isArray(face.subjects) && Array.isArray(face.properties) && Array.isArray(face.relationships));
    const unsupported = bindings.unsupported ?? [];
    assert.ok(Array.isArray(unsupported) && unsupported.length <= 1024);
    const declared = [...bindings.text_entries, ...bindings.collections.map(binding => binding.subject), ...unsupported.map(binding => binding.subject)];
    assert.equal(new Set(declared).size, declared.length, 'Duplicate prepared or unsupported subject');
    for (const subject of face.subjects.filter(subject => ['TextEntry', 'Collection'].includes(subject.role))) {
      assert.ok(declared.includes(subject.identity), 'Every portable control needs preparation or explicit refusal');
    }
    const property = (subject, name) => face.properties.find(item => item.subject === subject && item.name === name)?.value;
    const fields = bindings.text_entries.map(subject => ({ subject, value: property(subject, 'current-value')?.TypedValue }));
    const collections = bindings.collections.map(binding => ({
      subject: binding.subject, mode: binding.mode,
      items: face.relationships.filter(relation => relation.source === binding.subject && relation.kind === 'Contains')
        .map(relation => ({ identity: relation.target, order: property(relation.target, 'order')?.Count }))
        .sort((a, b) => a.order - b.order).map(item => item.identity),
    }));
    const suggestionSources = new Set(bindings.text_entries.map(subject => property(subject, 'suggestion-source')?.Identity));
    return { record, face, fields, collections,
      renderedCollections: collections.filter(collection => !suggestionSources.has(collection.subject)),
      selections: face.properties.filter(property => property.name === 'selected').map(property => ({ subject: property.subject, selected: property.value.Flag })),
      unsupportedExclusive: collections.filter(collection => collection.mode === 'exclusive' && !suggestionSources.has(collection.subject)).map(collection => collection.subject),
    };
  });
}

export async function reconstruct(capture, runtime) {
  const records = validateCapture(capture);
  assert.ok(runtime && path.isAbsolute(runtime));
  const require = createRequire(path.join(runtime, 'package.json'));
  assert.equal(require('@playwright/test/package.json').version, '1.62.0');
  const { chromium } = require('@playwright/test');
  const assets = new Map(await Promise.all(['controls', 'model', 'combobox'].map(async name => {
    const basename = `portable-face-${name}.mjs`;
    return [`/${basename}`, await readFile(new URL(`../../targets/browser/host/assets/${basename}`, import.meta.url), 'utf8')];
  })));
  const browser = await chromium.launch({ headless: true });
  const results = [];
  try {
    const page = await browser.newPage();
    await page.route('http://native-face.test/**', route => {
      const pathname = new URL(route.request().url()).pathname;
      if (pathname !== '/' && !assets.has(pathname)) return route.abort();
      return route.fulfill({ contentType: assets.has(pathname) ? 'text/javascript' : 'text/html', body: assets.get(pathname)
        ?? '<!doctype html><html lang="en"><title>Captured native Face: readonly reconstruction</title><body></body></html>' });
    });
    await page.goto('http://native-face.test/');
    for (const { record, face, fields, collections, renderedCollections, selections, unsupportedExclusive } of records) {
      const observed = await page.evaluate(async record => {
        const { renderPortableFace } = await import('/portable-face-controls.mjs');
        const { portableFaceModel } = await import('/portable-face-model.mjs');
        globalThis.submissions = 0;
        const encounter = { face: JSON.parse(record.face_json), control_bindings: record.control_bindings,
          view: { face_id: record.face_id, face_revision: record.face_revision,
            show_state: 'unavailable', interactions_admitted: false } };
        const before = JSON.stringify(encounter.face);
        const model = portableFaceModel(encounter);
        globalThis.readonlySurface?.retire();
        globalThis.readonlySurface = renderPortableFace(document.body, encounter, {
          enhanced: false, submit() { globalThis.submissions++; throw new Error('Historical Face has no submission authority'); },
        });
        // Deliberately never activate: this reconstruction has no acknowledged Show.
        const controls = [...document.querySelectorAll('button,input,select,textarea')];
        for (const details of document.querySelectorAll('details')) details.open = true;
        return { canonical: document.querySelector('[aria-label="Canonical Face facts"]').textContent,
          after: JSON.stringify(encounter.face), before, disabled: controls.every(control => control.disabled),
          fields: model.fields.map(field => ({ subject: field.subject.identity, value: { contract: field.value.contract, bytes: field.value.bytes } })),
          collections: model.collections.map(collection => ({ subject: collection.subject.identity, mode: collection.mode, items: collection.items.map(item => item.identity) })),
          fieldValues: model.fields.map(field => {
            const input = document.querySelector(`[data-source-subject="${CSS.escape(field.subject.identity)}"]`);
            return { subject: field.subject.identity, value: input?.value,
              expected: field.policy === 'fixed' ? field.options.find(option => JSON.stringify(option.bytes) === JSON.stringify(field.value.bytes))?.identity ?? '' : field.value.text };
          }),
          renderedCollections: [...document.querySelectorAll('[data-source-collection]')].map(section => ({
            subject: section.dataset.sourceCollection,
            items: [...section.querySelectorAll('[data-source-item]')].map(item => item.dataset.sourceItem),
          })),
          selections: [...document.querySelectorAll('input[type=checkbox]')].map(input => ({
            subject: input.closest('[data-source-item]').dataset.sourceItem, selected: input.checked,
          })),
          submissions: globalThis.submissions }; 
      }, record);
      assert.equal(observed.before, record.face_json);
      assert.equal(observed.after, record.face_json);
      assert.equal(observed.canonical, record.face_json);
      assert.equal(observed.disabled, true);
      assert.deepEqual(observed.fields, fields);
      assert.deepEqual(observed.collections, collections);
      assert.deepEqual(observed.renderedCollections, renderedCollections.map(({ subject, items }) => ({ subject, items })));
      for (const selection of observed.selections) assert.deepEqual(selection, selections.find(value => value.subject === selection.subject));
      for (const value of observed.fieldValues) assert.equal(value.value, value.expected);
      assert.equal(observed.submissions, 0);
      // Inspect only: no forced clicks, dispatchEvent, activation, or polling.
      assert.equal(await page.evaluate(() => globalThis.submissions), 0);
      results.push({ face_id: face.identity, face_revision: record.face_revision, face_sha256: record.face_sha256,
        unsupported_exclusive_interaction: unsupportedExclusive, unsupported_profiles: record.control_bindings.unsupported ?? [] });
    }
  } finally { await browser.close(); }
  return { schema: 'conduit.proof/native-face-browser-readonly@1', proof_class: 'historical-readonly-dom-reconstruction',
    source: capture.source, image: capture.image, browser_show: null, input_enactment: false, submissions: 0,
    resources: [...assets].map(([resource, bytes]) => ({ resource, sha256: digest(bytes), bytes: Buffer.byteLength(bytes) })), records: results };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const capturePath = process.env.CONDUIT_NATIVE_FACE_BROWSER_RECORDS;
  assert.ok(capturePath && path.isAbsolute(capturePath));
  const bytes = await readFile(capturePath);
  assert.ok(bytes.length <= 32 * 1024 * 1024, 'Capture envelope capacity');
  const receipt = await reconstruct(JSON.parse(bytes), process.env.CONDUIT_BROWSER_RUNTIME_ROOT);
  console.log(JSON.stringify(receipt));
}
