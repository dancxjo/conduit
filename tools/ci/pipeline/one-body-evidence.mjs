import { lstatSync, readFileSync } from 'node:fs';
import path from 'node:path';

export const ONE_BODY_EVIDENCE_ROOT = 'site/evidence/one-body-five-masks';

export function retainedOneBodyEvidence() {
  let root;
  try {
    root = lstatSync(ONE_BODY_EVIDENCE_ROOT);
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
  if (!root.isDirectory() || root.isSymbolicLink()) {
    throw new Error('One Body evidence root must be a regular source directory');
  }
  const manifest = JSON.parse(readFileSync(path.join(ONE_BODY_EVIDENCE_ROOT, 'manifest.json'), 'utf8'));
  const sourceCommit = manifest.git_commit;
  if (!/^[a-f0-9]{40}$/.test(sourceCommit)
    || manifest.schema !== 'conduit.evidence-manifest/v1'
    || manifest.result !== 'complete'
    || manifest.proof_id !== 'journey-one-body-five-masks'
    || manifest.suite_id !== 'journey-gallery') {
    throw new Error('One Body evidence lacks a complete, exact-source journey manifest');
  }
  return { root: ONE_BODY_EVIDENCE_ROOT, sourceCommit };
}
