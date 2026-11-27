// Fresh WASM membership and ordinary UI against the retained genuine Owner.
// The producer is a sealed original-recording CPU fixture, not physical capture.
import { test, expect } from '@playwright/test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { startStaticProduct } from './static-product-server.mjs';
import { parseCaptureJSON, captureDecimal } from './exact-capture-json.mjs';
import { assertCompleteReading } from './environment-recording-reading.mjs';

test('complete original readings traverse a live Owner and fresh ordinary WASM Mask', async ({ page }) => {
  const required = name => {
    const value = process.env[name];
    assert.ok(value && path.isAbsolute(value), `${name} must be an absolute fixture path`);
    return value;
  };
  const service = required('CONDUIT_RECORDING_OWNER_SERVICE');
  const handbook = required('CONDUIT_RECORDING_HANDBOOK_PACKAGE');
  const output = required('CONDUIT_RECORDING_BROWSER_OUTPUT');
  const native = required('CONDUIT_RECORDING_NATIVE_ABI_PROOF');
  const nativeFace = parseCaptureJSON(await readFile(path.join(native, 'canonical-face.json'), 'utf8'));
  const nativeView = parseCaptureJSON(await readFile(path.join(native, 'available-view.json'), 'utf8'));
  const ready = JSON.parse(await readFile(path.join(service, 'ready.json'), 'utf8'));
  const command = (verb, extra = []) => {
    const result = spawnSync(ready.binary, ['body', verb, '--state-dir', ready.state_dir, ...extra], {
      encoding: 'utf8', timeout: 15_000, maxBuffer: 2 * 1024 * 1024,
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    return parseCaptureJSON(result.stdout);
  };
  await mkdir(output, { recursive: false, mode: 0o700 });
  const errors = [];
  const frames = [];
  const traffic = [];
  const note = entry => { if (traffic.length < 1000) traffic.push(entry); };
  const returnedShows = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text().slice(0, 2000)); });
  page.on('websocket', socket => {
    note({ event: 'socket-created', url: socket.url() });
    socket.on('socketerror', error => note({ event: 'socket-error', error: String(error).slice(0, 2000) }));
    socket.on('close', () => note({ event: 'socket-closed' }));
    let returned;
    socket.on('framesent', ({ payload }) => {
      const text = typeof payload === 'string' ? payload : payload.toString('utf8');
      try {
        const frame = parseCaptureJSON(text);
        note({ event: 'sent', kind: frame.kind, bytes: Buffer.byteLength(text) });
        if (frame.kind === 'face-show-acknowledgement') returned = { text, frame };
      } catch { /* Other protocol messages cannot qualify a Show. */ }
    });
    socket.on('framereceived', ({ payload }) => {
      const text = typeof payload === 'string' ? payload : payload.toString('utf8');
      // Retain actual carrier bytes, with no CPU Host/Boot or view replay.
      try {
        const frame = parseCaptureJSON(text);
        note({ event: 'received', kind: frame.kind, accepted: frame.accepted, code: frame.code, bytes: Buffer.byteLength(text) });
        if (frame.kind === 'face-snapshot-response' && frame.response?.outcome === 'snapshot') {
          frames.push({ text, frame });
        } else if (frame.kind === 'face-show-response' && returned) {
          returnedShows.push({ ...returned, response: frame });
          returned = null;
        }
      } catch { /* Unrelated carrier frames are outside this proof. */ }
    });
  });
  const server = await startStaticProduct(handbook);
  let started = false;
  try {
    await page.addInitScript(() => {
      globalThis.__recordingFaceDiagnostics = [];
      new MutationObserver(() => {
        const node = document.querySelector('[data-owner-face-status]');
        const text = node?.textContent;
        const history = globalThis.__recordingFaceDiagnostics;
        if (text && history.length < 100 && history.at(-1) !== text) history.push(text);
      }).observe(document, { subtree: true, childList: true, characterData: true });
    });
    await page.goto(`${server.url}?participate=owner#your-handbook`);
    await page.locator('[data-owner-key]').waitFor();
    const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
    const window = command('browser-window', [
      '--expected-host-id', identity.hostId,
      '--new-host-verifying-key', JSON.stringify(identity.verifyingKey),
      '--maximum-millis', '60000', '--authorize-window',
    ]);
    assert.equal(window.body_id, ready.body_id);
    await page.getByLabel('Body ID').fill(ready.body_id);
    await page.getByLabel('Owner window URL').fill(window.url);
    await page.getByRole('button', { name: 'Join this Body' }).click();
    await expect.poll(() => page.evaluate(() => globalThis.__conduitOwnerParticipation.presence()))
      .toBe('available');
    // Membership precedes Play. The normal actor owns the Source worker.
    command('start', ['--maximum-millis', '30000']);
    started = true;
    await expect.poll(() => {
      const face = command('face', ['--json']).face;
      return face.properties.filter(p => p.name === 'visible-samples' && p.value?.Count === 64).length;
    }, { timeout: 15_000, intervals: [250, 500] }).toBe(2);
    await expect(page.locator('[data-owner-face-refresh]')).toBeEnabled();
    await page.locator('[data-owner-face-refresh]').click();
    await expect.poll(() => page.evaluate(() => {
      const view = globalThis.__conduitOwnerParticipation.face();
      return view?.show_state === 'available'
        && view.subjects?.filter(s => s.quantity_series?.plot?.source_samples === 64).length === 2;
    }), { timeout: 15_000 }).toBe(true);
    const occurrence = await page.evaluate(() => {
      const api = globalThis.__conduitOwnerParticipation;
      return { view: api.face(), canonical: api.canonicalFaceJSON(), credential: api.credential(), host: api.host() };
    });
    const canonical = parseCaptureJSON(occurrence.canonical);
    const source = frames.findLast(({ frame }) => frame.response.face.identity === canonical.face.identity
      && captureDecimal(frame.response.face.revision) === canonical.face_revision_decimal);
    assert.ok(source, 'canonical Face must match a freshly received genuine owner frame');
    assert.deepEqual(canonical.face, source.frame.response.face);
    const acknowledged = returnedShows.findLast(item =>
      item.frame.show.show_id === occurrence.view.show_id && item.response.accepted === true);
    assert.ok(acknowledged, 'Owner must accept this exact returned Show on the live carrier');
    assert.equal(acknowledged.frame.show.face_id, canonical.face.identity);
    assert.equal(captureDecimal(acknowledged.frame.show.face_revision), canonical.face_revision_decimal);

    assert.equal(canonical.face.basis.body_id, ready.body_id);
    assert.equal(source.frame.response.route.mask_host.host_id, identity.hostId);
    assert.equal(source.frame.response.route.mask_host.boot_id, occurrence.credential.boot_id);
    assert.equal(occurrence.view.route.mask_host_id, identity.hostId);
    assert.equal(occurrence.view.route.owner_host_id, ready.owner_host_id);
    assert.equal(occurrence.view.route.owner_boot_id, ready.owner_boot_id);
    assert.equal(occurrence.view.subjects.length, canonical.face.subjects.length);
    assert.equal(occurrence.view.relationships.length, canonical.face.relationships.length);
    assert.equal(occurrence.view.actions.length, canonical.face.actions.length);
    for (const reading of occurrence.view.subjects) {
      assert.equal(reading.properties.length,
        canonical.face.properties.filter(p => p.subject === reading.identity).length);
    }
    assertCompleteReading(canonical.face, occurrence.view, nativeFace, nativeView);
    const series = occurrence.view.subjects.filter(s => s.quantity_series?.plot);
    assert.equal(series.length, 2);
    await expect(page.locator('.owner-face-series')).toHaveCount(2);
    await expect(page.locator('.owner-face-series svg circle')).toHaveCount(32);
    for (const reading of series) {
      assert.equal(reading.quantity_series.plot.source_samples, 64);
      assert.equal(reading.quantity_series.plot.points.length, 16);
      assert.equal(reading.quantity_series.plot.omitted_points, 16);
      assert.equal(reading.quantity_series.plot.unavailable_samples, 32);
    }
    assert.ok(Buffer.byteLength(JSON.stringify(canonical.face)) > 60 * 1024);
    assert.deepEqual(errors, []);
    await writeFile(path.join(output, 'owner-frame.json'), source.text);
    await writeFile(path.join(output, 'canonical-face-envelope.json'), occurrence.canonical);
    await writeFile(path.join(output, 'available-view.json'), JSON.stringify(occurrence.view));
    await writeFile(path.join(output, 'returned-show.json'), acknowledged.text);
    await writeFile(path.join(output, 'owner-show-response.json'), JSON.stringify(acknowledged.response));
    await writeFile(path.join(output, 'result.json'), JSON.stringify({
      schema: 'conduit.proof/environment-recording-owner-browser@1',
      accepted: true, body_id: ready.body_id, browser_host_id: identity.hostId,
      browser_boot_id: occurrence.credential.boot_id,
      face_id: canonical.face.identity, face_revision: canonical.face_revision_decimal,
      show_id: occurrence.view.show_id, chromium: true, wasm: true,
      physical_sensor: false, recorder_acknowledgement: false, released_acceptance: false,
    }));
  } finally {
    try {
      const state = await page.evaluate(() => {
        const api = globalThis.__conduitOwnerParticipation;
        return { state: api?.state(), presence: api?.presence(), host: api?.host(),
          status: document.querySelector('[data-owner-status]')?.textContent,
          faceHistory: globalThis.__recordingFaceDiagnostics };
      }).catch(error => ({ error: error.message }));
      await writeFile(path.join(output, 'diagnostic.json'), JSON.stringify({ errors, traffic, state }, null, 2));
    } finally {
      try {
        if (started) command('lull');
      } finally {
        try { await writeFile(path.join(service, 'stop'), 'stop'); }
        finally { server.child.kill(); }
      }
    }
  }
});
