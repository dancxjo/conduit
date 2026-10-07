// Capture one browser-visible result at the moment its real producer action
// finishes. These observations are inputs to a later journey bundle, not a
// substitute for the complete eight-chapter acceptance receipt.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { lstat, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

export async function recordBrowserCapture({ output, name, sourceCommit, runId, bodyId,
  hostId, bootId, face, cause, screenshot }) {
  assert.match(name, /^[a-z][a-z0-9-]{0,63}$/);
  assert.match(screenshot, /^[a-z][a-z0-9-]*\.png$/);
  assert.match(sourceCommit, /^[a-f0-9]{40}$/);
  for (const value of [runId, bodyId, hostId, bootId, face?.face_id,
    face?.face_revision, face?.show_id]) {
    assert.ok(value !== undefined && value !== null && value !== '',
      'capture identity is missing');
  }
  assert.equal(face.body_id, bodyId);
  assert.ok(cause && typeof cause === 'object' && typeof cause.kind === 'string');
  const screenshotPath = path.join(output, screenshot);
  const source = await lstat(screenshotPath);
  assert.ok(source.isFile() && !source.isSymbolicLink(), 'capture is not a regular file');
  assert.ok(source.size <= 16 * 1024 * 1024, 'capture exceeds 16 MiB');
  const bytes = await readFile(screenshotPath);
  assert.ok(bytes.subarray(0, 8).equals(pngSignature), 'capture is not a PNG');
  const observation = {
    schema: 'conduit.proof/browser-capture-observation@1',
    source_commit: sourceCommit,
    run_id: runId,
    body_id: bodyId,
    browser_host_id: hostId,
    browser_boot_id: bootId,
    cause,
    resulting_face: { face_id: face.face_id, face_revision: face.face_revision,
      show_id: face.show_id },
    capture: { source: 'pinned-chromium', path: screenshot, bytes: bytes.length,
      sha256: digest(bytes) },
  };
  await mkdir(path.join(output, 'observations'), { recursive: true });
  const relative = `observations/${name}.json`;
  const encoded = Buffer.from(`${JSON.stringify(observation, null, 2)}\n`);
  assert.ok(encoded.length <= 64 * 1024, 'capture observation exceeds 64 KiB');
  await writeFile(path.join(output, relative), encoded, { flag: 'wx', mode: 0o600 });
  return { path: relative, bytes: encoded.length, sha256: digest(encoded) };
}
