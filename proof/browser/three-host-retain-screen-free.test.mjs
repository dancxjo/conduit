import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { retainScreenFreeSessions } from './three-host-retain-screen-free.mjs';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');

async function fixture() {
  const root = await mkdtemp(path.join(tmpdir(), 'conduit-screen-free-retain-'));
  const walkthrough = path.join(root, 'three-host');
  await mkdir(walkthrough);
  const artifact = async name => {
    const bytes = Buffer.from(`observed ${name}\n`);
    await writeFile(path.join(root, name), bytes);
    return { path: `../${name}`, bytes: bytes.length, sha256: digest(bytes) };
  };
  const report = {
    birth: {
      zero_body_receipt: await artifact('zero-body-before.json'),
      input: await artifact('birth-input.txt'),
      transcript: await artifact('birth-transcript.txt'),
    },
    screen_free_clock: {
      start: {
        input: await artifact('clock-start-input.txt'),
        transcript: await artifact('clock-start-transcript.txt'),
      },
      lull: {
        input: await artifact('clock-lull-input.txt'),
        transcript: await artifact('clock-lull-transcript.txt'),
      },
      wake: {
        input: await artifact('clock-wake-input.txt'),
        transcript: await artifact('clock-wake-transcript.txt'),
      },
      finish: {
        input: await artifact('clock-finish-input.txt'),
        transcript: await artifact('clock-finish-transcript.txt'),
      },
    },
    screen_free_wardrobe: {
      provider_transcript: await artifact('wardrobe-provider-transcript.txt'),
      input: await artifact('wardrobe-input.txt'),
      transcript: await artifact('wardrobe-transcript.txt'),
    },
  };
  const checkpointInput = 'screen-free-checkpoint-browser-presentation-unavailable-input.txt';
  const checkpointTranscript = 'screen-free-checkpoint-browser-presentation-unavailable-transcript.txt';
  const liveArtifact = async name => {
    const bytes = Buffer.from(`observed ${name}\n`);
    await writeFile(path.join(walkthrough, name), bytes);
    return { path: name, bytes: bytes.length, sha256: digest(bytes) };
  };
  report.screen_free_checkpoints = { checkpoints: [{
    phase: 'browser-presentation-unavailable',
    input: await liveArtifact(checkpointInput),
    transcript: await liveArtifact(checkpointTranscript),
  }] };
  return { root, walkthrough, report };
}

test('retains only verified nonvisual sessions beside the walkthrough', async () => {
  const { root, walkthrough, report } = await fixture();
  try {
    await writeFile(path.join(root, 'invitation.private.json'), 'secret');
    await retainScreenFreeSessions(root, walkthrough, report);
    const names = await readdir(walkthrough);
    assert.equal(names.length, 16);
    assert.ok(!names.includes('invitation.private.json'));
    assert.equal(report.birth.input.path, 'birth-input.txt');
    assert.equal(report.screen_free_clock.lull.transcript.path, 'clock-lull-transcript.txt');
    assert.equal(report.screen_free_clock.finish.transcript.path, 'clock-finish-transcript.txt');
    assert.equal(report.screen_free_wardrobe.transcript.path, 'wardrobe-transcript.txt');
    assert.equal(report.screen_free_checkpoints.checkpoints[0].transcript.path,
      'screen-free-checkpoint-browser-presentation-unavailable-transcript.txt');
    assert.equal(digest(await readFile(path.join(walkthrough, 'birth-input.txt'))),
      report.birth.input.sha256);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('refuses a changed transcript before retaining it', async () => {
  const { root, walkthrough, report } = await fixture();
  try {
    await writeFile(path.join(root, 'zero-body-before.json'), 'changed');
    await assert.rejects(retainScreenFreeSessions(root, walkthrough, report),
      /changed after its receipt/);
    assert.deepEqual((await readdir(walkthrough)).sort(), [
      'screen-free-checkpoint-browser-presentation-unavailable-input.txt',
      'screen-free-checkpoint-browser-presentation-unavailable-transcript.txt',
    ]);
  } finally { await rm(root, { recursive: true, force: true }); }
});
