import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { retainWorkspaceEvidence } from '../../tools/ci/pipeline/targets/workspace-evidence.mjs';

test('retains original Workspace captures and rejects incomplete or ambiguous runs', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'workspace-retention-'));
  try {
    const results = path.join(root, 'results');
    const source = path.join(results, 'chromium');
    mkdirSync(source, { recursive: true });
    const actions = [{ id: 'arrive', capture: '01-arrive' }];
    const report = path.join(source, 'workspace-proof.json');
    const proof = { schema: 'conduit.browser/workspace-journey-proof@1', observations: [{ action: 'arrive' }] };
    writeFileSync(report, JSON.stringify(proof));
    const fixture = Buffer.from('fixture bytes, not a runtime screenshot');
    writeFileSync(path.join(source, '01-arrive.png'), fixture);
    const destination = path.join(root, 'retained');
    retainWorkspaceEvidence(results, destination, actions);
    assert.deepEqual(readFileSync(path.join(destination, '01-arrive.png')), fixture);
    assert.deepEqual(readFileSync(path.join(destination, 'workspace-proof.json')), readFileSync(report));
    proof.observations = [];
    writeFileSync(report, JSON.stringify(proof));
    assert.throws(() => retainWorkspaceEvidence(results, path.join(root, 'incomplete'), actions), /shared action/);
    writeFileSync(path.join(results, 'workspace-proof.json'), JSON.stringify(proof));
    assert.throws(() => retainWorkspaceEvidence(results, path.join(root, 'ambiguous'), actions), /one current/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
