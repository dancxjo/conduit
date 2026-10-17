import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { retainedThreeHostDevelopmentEvidence } from '../../tools/ci/pipeline/three-host-development-evidence.mjs';

test('development evidence is absent until a retained diagnostic manifest exists', () => {
  const parent = mkdtempSync(path.join(os.tmpdir(), 'conduit-three-host-evidence-'));
  try {
    assert.equal(retainedThreeHostDevelopmentEvidence(path.join(parent, 'missing')), null);
    const root = path.join(parent, 'evidence');
    mkdirSync(root);
    const manifest = {
      schema: 'conduit.evidence-manifest/v1', result: 'diagnostic-incomplete',
      git_commit: 'a'.repeat(40), proof_id: 'journey-one-body-three-host-development',
      suite_id: 'journey-gallery', outputs: [
        { path: 'index.html' }, { path: 'report.json' },
        { path: 'browser.png' }, { path: 'speaker.wav' },
      ],
    };
    writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
    writeFileSync(path.join(root, 'report.json'), JSON.stringify({
      schema: 'conduit.body/three-host-owner-journey@1',
      native_source_commit: 'a'.repeat(40), run_id: 'run-1', body_id: 'body-1',
      owner_llm_speech: { wav: { path: 'speaker.wav' } },
    }));
    const chapters = ['birth', 'join', 'start', 'see', 'hear', 'loss', 'return', 'lull']
      .map(id => `<article id="${id}"></article>`).join('');
    const page = links => `Development capture · 4 of 8 chapters complete. `
      + 'The public Journey and accepted release require separate gates. '
      + chapters + links;
    writeFileSync(path.join(root, 'index.html'), page(
      '<a href="/conduit/">Home</a><img src="browser.png"><audio src="speaker.wav"></audio>'));
    assert.deepEqual(retainedThreeHostDevelopmentEvidence(root), {
      root, sourceCommit: 'a'.repeat(40),
    });
    writeFileSync(path.join(root, 'index.html'), page(
      '<a href="/conduit/unreviewed/">Other</a><img src="browser.png"><audio src="speaker.wav"></audio>'));
    assert.throws(() => retainedThreeHostDevelopmentEvidence(root), /undeclared asset/);
    writeFileSync(path.join(root, 'index.html'), page(
      '<a href="/conduit/">Home</a><img src="browser.png"><audio src="speaker.wav"></audio>'));
    manifest.result = 'complete';
    writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
    assert.throws(() => retainedThreeHostDevelopmentEvidence(root), /exact-source diagnostic/);
    manifest.result = 'diagnostic-incomplete';
    manifest.outputs.pop();
    writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
    assert.throws(() => retainedThreeHostDevelopmentEvidence(root), /eight-chapter diagnostic/);
    writeFileSync(path.join(root, 'report.json'), JSON.stringify({
      schema: 'conduit.body/three-host-owner-journey@1',
      native_source_commit: 'a'.repeat(40), run_id: 'run-1', body_id: 'body-1',
    }));
    writeFileSync(path.join(root, 'index.html'), page('<img src="browser.png">'));
    assert.deepEqual(retainedThreeHostDevelopmentEvidence(root), {
      root, sourceCommit: 'a'.repeat(40),
    });
  } finally {
    rmSync(parent, { recursive: true, force: true });
  }
});
