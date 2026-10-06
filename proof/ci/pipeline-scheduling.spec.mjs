import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import test from 'node:test';

const ci = readFileSync('.github/workflows/ci.yml', 'utf8');
const candidate = readFileSync('.github/workflows/candidate.yml', 'utf8');
const integration = readFileSync('.github/workflows/integration.yml', 'utf8');
function job(source, name) {
  return source.split(`\n  ${name}:\n`)[1]?.split(/\n  [a-z]+:\n/)[0];
}

test('independent proof starts after preflight and preserves complete aggregation', () => {
  for (const name of ['unit', 'target']) {
    assert.match(job(ci, name), /needs: preflight\n/);
    assert.match(job(ci, name), /if: inputs.exhaustive && needs.preflight.outputs.docs-only != 'true'/);
    assert.match(job(ci, name), /fail-fast: false/);
  }
  assert.match(job(ci, 'complete'), /needs: \[preflight, unit, target\]/);
  assert.match(job(ci, 'complete'), /if: \$\{\{ always\(\) \}\}/);
});

test('draft transitions run quick checks and ready transitions require exact-head proof', () => {
  assert.match(candidate, /types: \[opened, synchronize, reopened, ready_for_review, converted_to_draft\]/);
  assert.match(job(candidate, 'proof'), /exhaustive: \$\{\{ !github.event.pull_request.draft \}\}/);
  assert.match(job(candidate, 'proof'), /sha: \$\{\{ github.event.pull_request.head.sha \}\}/);
  assert.match(job(candidate, 'candidate'), /always\(\) && !github.event.pull_request.draft/);
  assert.match(job(candidate, 'candidate'), /needs: proof/);
  assert.match(job(candidate, 'candidate'), /run: test "\$RESULT" = success/);
  assert.match(job(ci, 'preflight'), /name: candidate\/quick/);
  assert.match(ci, /exhaustive:\n        type: boolean\n        default: true/);
  assert.match(integration, /base: all/);
  assert.doesNotMatch(integration, /exhaustive: false/);
  assert.match(integration, /cancel-in-progress: false/);
});

test('the actual AND gate rejects every failed, cancelled, or missing selected lane', () => {
  const script = job(ci, 'complete').split('        run: |\n')[1].replace(/^          /gm, '');
  const base = { PREFLIGHT: 'success', EXHAUSTIVE: 'true', DOCS_ONLY: 'false', UNIT: 'success', TARGET: 'success' };
  function passes(overrides) {
    return spawnSync('bash', ['-e', '-c', script], { env: { ...process.env, ...base, ...overrides } }).status === 0;
  }
  assert.equal(passes({}), true);
  for (const lane of ['PREFLIGHT', 'UNIT', 'TARGET']) {
    for (const result of ['failure', 'cancelled', 'skipped', '']) assert.equal(passes({ [lane]: result }), false, `${lane}=${result}`);
  }
  assert.equal(passes({ DOCS_ONLY: 'true', UNIT: 'skipped', TARGET: 'skipped' }), true);
  assert.equal(passes({ EXHAUSTIVE: 'false', UNIT: 'skipped', TARGET: 'skipped' }), true);
  assert.equal(passes({ EXHAUSTIVE: 'false', PREFLIGHT: 'failure' }), false);
});
