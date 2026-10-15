import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import test from 'node:test';

const ci = readFileSync('.github/workflows/ci.yml', 'utf8');
const candidate = readFileSync('.github/workflows/candidate.yml', 'utf8');
const integration = readFileSync('.github/workflows/integration.yml', 'utf8');
function job(source, name) {
  return source.split(`\n  ${name}:\n`)[1]?.split(/\n  [A-Za-z_][A-Za-z0-9_-]*:\n/)[0];
}

test('independent proof starts after preflight and preserves complete aggregation', () => {
  for (const name of ['unit', 'target']) {
    assert.match(job(ci, name), /needs: preflight\n/);

    assert.match(job(ci, name), /fail-fast: false/);
  }
  assert.match(job(ci, 'unit'), /matrix: \$\{\{ fromJSON\(needs.preflight.outputs.unit-matrix\) \}\}/);
  assert.match(job(ci, 'complete'), /needs: \[preflight, unit, admission, target\]/);
  assert.match(job(ci, 'complete'), /if: \$\{\{ always\(\) && !cancelled\(\) \}\}/);
});

test('code-changing PR events run exact proof without restarting on visibility changes', () => {
  const events = new Set(candidate.match(/types:\s*\[([^\]]+)\]/)[1].split(/\s*,\s*/));
  for (const event of ['opened', 'synchronize', 'reopened']) assert.ok(events.has(event));
  for (const event of ['ready_for_review', 'converted_to_draft']) assert.ok(!events.has(event));
  assert.match(job(candidate, 'proof'), /exhaustive: false/);
  assert.match(job(candidate, 'proof'), /sha: \$\{\{ github.sha \}\}/);
  assert.match(job(candidate, 'candidate'), /if: \$\{\{ always\(\) && !cancelled\(\) \}\}/);
  assert.match(job(candidate, 'candidate'), /needs: proof/);
  assert.match(job(candidate, 'candidate'), /test "\$RESULT" = success/);
  assert.match(job(ci, 'preflight'), /name: candidate\/quick/);
  assert.match(ci, /exhaustive:\n        type: boolean\n        default: true/);
  assert.match(integration, /base: all/);
  assert.doesNotMatch(integration, /exhaustive: false/);
  assert.match(integration, /cancel-in-progress: false/);
});

test('the actual AND gate rejects every failed, cancelled, or missing selected lane', () => {
  const script = job(ci, 'complete').split('        run: |\n')[1].replace(/^          /gm, '');
  const base = { PREFLIGHT: 'success', EXHAUSTIVE: 'true', DOCS_ONLY: 'false', UNIT: 'success', TARGET: 'success', ADMISSION: 'success', HAS_TARGETS: 'true' };
  function passes(overrides) {
    return spawnSync('bash', ['-e', '-c', script], { env: { ...process.env, ...base, ...overrides } }).status === 0;
  }
  assert.equal(passes({}), true);
  for (const lane of ['PREFLIGHT', 'UNIT', 'TARGET']) {
    for (const result of ['failure', 'cancelled', 'skipped', '']) assert.equal(passes({ [lane]: result }), false, `${lane}=${result}`);
  }
  assert.equal(passes({ DOCS_ONLY: 'true', UNIT: 'skipped', TARGET: 'skipped', HAS_TARGETS: 'false' }), true);
  assert.equal(passes({ EXHAUSTIVE: 'false', UNIT: 'skipped', TARGET: 'skipped', HAS_TARGETS: 'false' }), true);
  assert.equal(passes({ EXHAUSTIVE: 'false', PREFLIGHT: 'failure' }), false);
  for (const result of ['failure', 'cancelled', 'skipped', '']) {
    assert.equal(passes({ EXHAUSTIVE: 'false', ADMISSION: result }), false);
    assert.equal(passes({ EXHAUSTIVE: 'false', TARGET: result }), false);
  }
});


test('required candidate accepts healthy drafts and rejects unsuccessful proof', () => {
  const script = job(candidate, 'candidate').split('        run: |\n')[1].replace(/^          /gm, '');
  function passes(draft, result) {
    return spawnSync('bash', ['-e', '-c', script], {
      env: { ...process.env, DRAFT: draft, RESULT: result },
    }).status === 0;
  }
  assert.equal(passes('false', 'success'), true);
  assert.equal(passes('true', 'success'), true);
  for (const result of ['failure', 'cancelled', 'skipped', '']) assert.equal(passes('true', result), false);
  for (const result of ['failure', 'cancelled', 'skipped', '']) assert.equal(passes('false', result), false);

});

test('PR scopes use the actual merge-preview parent, including admission', () => {
  assert.match(job(ci, 'preflight'), /proof_base=\$\(git rev-parse "\$SHA\^1"\)/);
  assert.match(job(ci, 'preflight'), /git merge-base --is-ancestor "\$BASE" "\$proof_base"/);
  assert.match(job(ci, 'preflight'), /BASE: \$\{\{ steps\.base\.outputs\.sha \}\}/);
  assert.match(job(ci, 'admission'), /BASE: \$\{\{ needs\.preflight\.outputs\.base \}\}/);
});
