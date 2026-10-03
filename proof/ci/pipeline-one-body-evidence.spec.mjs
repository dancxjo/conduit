import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { ONE_BODY_EVIDENCE_ROOT, retainedOneBodyEvidence } from '../../tools/ci/pipeline/one-body-evidence.mjs';

function inFreshCheckout(check) {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-one-body-site-'));
  const previous = process.cwd();
  try {
    process.chdir(root);
    check();
  } finally {
    process.chdir(previous);
    rmSync(root, { recursive: true, force: true });
  }
}

function retainManifest(manifest) {
  mkdirSync(ONE_BODY_EVIDENCE_ROOT, { recursive: true });
  writeFileSync(path.join(ONE_BODY_EVIDENCE_ROOT, 'manifest.json'), JSON.stringify(manifest));
}

test('ordinary site builds omit absent One Body evidence', () => inFreshCheckout(() => {
  assert.equal(retainedOneBodyEvidence(), null);
}));

test('publication refuses incomplete or unrelated One Body evidence', () => inFreshCheckout(() => {
  const valid = {
    schema: 'conduit.evidence-manifest/v1', result: 'complete',
    git_commit: 'a'.repeat(40), proof_id: 'journey-one-body-five-masks', suite_id: 'journey-gallery',
  };
  retainManifest({ ...valid, result: 'diagnostic-incomplete' });
  assert.throws(() => retainedOneBodyEvidence(), /complete, exact-source/);
  retainManifest({ ...valid, proof_id: 'unrelated-proof' });
  assert.throws(() => retainedOneBodyEvidence(), /complete, exact-source/);
  retainManifest({ ...valid, git_commit: 'short' });
  assert.throws(() => retainedOneBodyEvidence(), /complete, exact-source/);
  retainManifest(valid);
  assert.deepEqual(retainedOneBodyEvidence(), { root: ONE_BODY_EVIDENCE_ROOT, sourceCommit: 'a'.repeat(40) });
}));
