import assert from 'node:assert/strict';
import { cpSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { DIRECT_SPOKEN_DEVELOPMENT_ROOT, retainedDirectSpokenDevelopmentEvidence }
  from '../../tools/ci/pipeline/direct-spoken-development-evidence.mjs';

test('retained direct speech has 11 same-Play reading WAVs and one distinct opening', () => {
  const evidence = retainedDirectSpokenDevelopmentEvidence();
  assert.equal(evidence.sourceCommit, '2f1df6f640a4ad967d90d795bd47fe98b177851e');
  const report = JSON.parse(readFileSync(path.join(evidence.root, 'report.json')));
  assert.equal(report.batches.length, 11);
  assert.equal(report.selected_speaker.completed_segments, 41);
  assert.equal(report.human_listening_observed, false);
});

test('retained direct speech refuses changed PCM identity and inflated proof', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'conduit-direct-spoken-evidence-'));
  try {
    for (const name of ['manifest.json', 'report.json', 'index.html']) {
      cpSync(path.join(DIRECT_SPOKEN_DEVELOPMENT_ROOT, name), path.join(root, name));
    }
    const reportPath = path.join(root, 'report.json');
    const original = JSON.parse(readFileSync(reportPath));
    for (const batch of original.batches) {
      symlinkSync(path.resolve(DIRECT_SPOKEN_DEVELOPMENT_ROOT, batch.wav_artifact_id),
        path.join(root, batch.wav_artifact_id));
    }
    symlinkSync(path.resolve(DIRECT_SPOKEN_DEVELOPMENT_ROOT, original.opening.wav_artifact_id),
      path.join(root, original.opening.wav_artifact_id));
    assert.deepEqual(retainedDirectSpokenDevelopmentEvidence(root), {
      root, sourceCommit: original.capture_source_commit,
    });
    const changed = structuredClone(original);
    changed.batches[0].pcm_sha256 = '0'.repeat(64);
    writeFileSync(reportPath, JSON.stringify(changed));
    assert.throws(() => retainedDirectSpokenDevelopmentEvidence(root), /same-Play PCM/);
    changed.batches[0].pcm_sha256 = original.batches[0].pcm_sha256;
    changed.human_listening_observed = true;
    writeFileSync(reportPath, JSON.stringify(changed));
    assert.throws(() => retainedDirectSpokenDevelopmentEvidence(root), /completed-run identities/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
