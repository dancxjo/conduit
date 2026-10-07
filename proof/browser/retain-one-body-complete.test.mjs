import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { retainOneBodyComplete } from './retain-one-body-complete.mjs';

const sha = 'a'.repeat(40);
const runId = 'journey_test';
const bodyId = 'body/test';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const owned = { source_commit: sha, run_id: runId, body_id: bodyId };
const report = () => ({
  schema: 'conduit.body/three-host-owner-journey@1',
  proof_class: 'live-local-installed-owner-qmp-pinned-chromium-selected-model-speaker',
  native_source_commit: sha, run_id: runId, body_id: bodyId,
  concurrent_part_count: 3, qemu_alive_through_browser_actions: true,
  birth: { source_commit: sha, body_id: bodyId, confirmation_observed: true },
  screen_free_clock: { ...owned, finish: { action_id: 'finish' } },
  owner_direct_speech: { ...owned }, owner_llm_speech: { ...owned },
  owner_model_route_loss: { ...owned }, presentation_host_recovery: {},
});

async function fixture(callback) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'conduit-complete-carrier-'));
  const source = path.join(root, 'run', 'three-host');
  const output = path.join(root, 'retained');
  await mkdir(source, { recursive: true });
  try { await callback({ source, output }); }
  finally { await rm(root, { recursive: true, force: true }); }
}

test('a completed-looking development report cannot become a complete Journey without producer chapters',
  async () => fixture(async ({ source, output }) => {
    await writeFile(path.join(source, 'report.json'), JSON.stringify(report()));
    await assert.rejects(retainOneBodyComplete(path.dirname(source), output),
      /lacks eight publication_chapters/);
    await assert.rejects(readFile(path.join(output, 'manifest.json')), /ENOENT/);
  }));

test('publication output cannot be placed inside the private run',
  async () => fixture(async ({ source }) => {
    await assert.rejects(retainOneBodyComplete(path.dirname(source),
      path.join(path.dirname(source), 'three-host', 'public')),
    /outside the private producer run/);
  }));

test('publication copy refuses an event identity absent from its producer receipt',
  async () => fixture(async ({ source, output }) => {
    const item = report();
    const names = ['birth', 'join', 'start', 'see', 'hear', 'loss', 'return', 'lull'];
    const evidence = Buffer.from(JSON.stringify({
      ...owned, event_kind: 'membership', event_id: 'browser-part-id',
      resulting_face_revision: '1', observed_at_unix_ms: 1,
      outcome: 'completed',
    }));
    item.publication_chapters = names.map(id => ({ id, title: id, intention: id,
      action: id, result: id, why: id, next: id, limitations: ['Local proof only.'],
      events: [{ kind: 'membership', id: `${id}-event`, face_revision: '1',
        observed_at_unix_ms: 1,
        source_receipt: { path: 'event.json', sha256: digest(evidence) } }],
      media: [{ path: `${id}.png`, sha256: '0'.repeat(64),
        source_receipt: { path: 'capture.json', sha256: '0'.repeat(64) },
        event_id: `${id}-event`, face_revision: '1', alt: id,
        capture_source: 'chromium' }] }));
    await writeFile(path.join(source, 'report.json'), JSON.stringify(item));
    await writeFile(path.join(source, 'event.json'), evidence);
    await assert.rejects(retainOneBodyComplete(path.dirname(source), output),
      /source has no exact event identity/);
    await assert.rejects(readFile(path.join(output, 'manifest.json')), /ENOENT/);
  }));

test('a later chapter cannot move back before an earlier recorded event',
  async () => fixture(async ({ source, output }) => {
    const item = report();
    const ids = ['birth', 'join', 'start', 'see', 'hear', 'loss', 'return', 'lull'];
    const first = Buffer.from(JSON.stringify({ ...owned,
      event_kind: 'typed-interaction', event_id: 'birth-event',
      resulting_face_revision: '1', observed_at_unix_ms: 200,
      outcome: 'completed' }));
    const terminal = Buffer.from('Birth completed.\n');
    const capture = Buffer.from(JSON.stringify({ ...owned,
      event_kind: 'typed-interaction', event_id: 'birth-event',
      face_revision: '1', media_path: 'birth.txt',
      media_sha256: digest(terminal), capture_source: 'terminal' }));
    await writeFile(path.join(source, 'birth-event.json'), first);
    await writeFile(path.join(source, 'birth.txt'), terminal);
    await writeFile(path.join(source, 'capture.json'), capture);
    item.publication_chapters = ids.map((id, index) => ({ id,
      title: id, intention: id, action: id, result: id, why: id, next: id,
      limitations: ['Local proof only.'],
      events: [{ kind: 'typed-interaction', id: `${id}-event`,
        face_revision: '1', observed_at_unix_ms: index === 0 ? 200 : 100,
        source_receipt: { path: 'birth-event.json', sha256: digest(first) } }],
      media: [{ path: index === 0 ? 'birth.txt' : `${id}.png`,
        sha256: index === 0 ? digest(terminal) : '0'.repeat(64),
        source_receipt: { path: 'capture.json', sha256: digest(capture) },
        event_id: `${id}-event`, face_revision: '1', alt: id,
        capture_source: index === 0 ? 'terminal' : 'chromium' }] }));
    await writeFile(path.join(source, 'report.json'), JSON.stringify(item));
    await assert.rejects(retainOneBodyComplete(path.dirname(source), output),
      /breaks journey chronology/);
    await assert.rejects(readFile(path.join(output, 'manifest.json')), /ENOENT/);
  }));
