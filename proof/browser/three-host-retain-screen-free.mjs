// Retain only the public-safe, producer-observed nonvisual sessions beside the
// walkthrough. Invitation, spore, TLS, and owner-state files stay private.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');

export async function retainScreenFreeSessions(privateRoot, walkthroughRoot, report) {
  assert.ok(report.birth && report.screen_free_clock,
    'screen-free walkthrough needs completed Birth and clock sessions');
  const files = [
    [report.birth.zero_body_receipt, 'zero-body-before.json'],
    [report.birth.input, 'birth-input.txt'],
    [report.birth.transcript, 'birth-transcript.txt'],
    ...['start', 'lull'].flatMap(name => [
      [report.screen_free_clock[name].input, `clock-${name}-input.txt`],
      [report.screen_free_clock[name].transcript, `clock-${name}-transcript.txt`],
    ]),
  ];
  for (const [artifact, name] of files) {
    assert.equal(artifact.path, `../${name}`, `unexpected screen-free artifact ${name}`);
    const bytes = await readFile(path.join(privateRoot, name));
    assert.equal(digest(bytes), artifact.sha256, `${name} changed after its receipt`);
    if (artifact.bytes !== undefined) assert.equal(bytes.length, artifact.bytes);
    await writeFile(path.join(walkthroughRoot, name), bytes, { flag: 'wx', mode: 0o600 });
    artifact.path = name;
  }
}
