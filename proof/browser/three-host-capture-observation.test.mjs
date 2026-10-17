import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { recordBrowserCapture } from './three-host-capture-observation.mjs';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9WlRoiQAAAAASUVORK5CYII=', 'base64');

test('retains exact screenshot bytes and the observed action/Face correlation once', async () => {
  const output = await mkdtemp(path.join(tmpdir(), 'conduit-browser-observation-'));
  try {
    await writeFile(path.join(output, 'browser-after.png'), png);
    const request = { output, name: 'after-action', sourceCommit: 'a'.repeat(40),
      runId: 'run-1', bodyId: 'body-1', hostId: 'host-1', bootId: 'boot-1',
      face: { body_id: 'body-1', face_id: 'face-2', face_revision: 2, show_id: 'show-2' },
      cause: { kind: 'browser-semantic-action', action_id: 'action-1',
        source_face_revision: 1, source_show_id: 'show-1', outcome: 'accepted' },
      screenshot: 'browser-after.png' };
    const retained = await recordBrowserCapture(request);
    const bytes = await readFile(path.join(output, retained.path));
    const observation = JSON.parse(bytes);
    assert.equal(retained.sha256, digest(bytes));
    assert.equal(observation.capture.sha256, digest(png));
    assert.equal(observation.resulting_face.face_revision, 2);
    assert.equal(observation.cause.source_show_id, 'show-1');
    await assert.rejects(recordBrowserCapture(request), /EEXIST/);
  } finally { await rm(output, { recursive: true, force: true }); }
});

test('refuses a capture for a different Body', async () => {
  const output = await mkdtemp(path.join(tmpdir(), 'conduit-browser-observation-'));
  try {
    await writeFile(path.join(output, 'browser-after.png'), png);
    await assert.rejects(recordBrowserCapture({ output, name: 'wrong-body',
      sourceCommit: 'b'.repeat(40), runId: 'run-1', bodyId: 'body-1',
      hostId: 'host-1', bootId: 'boot-1',
      face: { body_id: 'body-2', face_id: 'face-2', face_revision: 2, show_id: 'show-2' },
      cause: { kind: 'browser-membership' }, screenshot: 'browser-after.png' }),
    /body-1/);
  } finally { await rm(output, { recursive: true, force: true }); }
});
