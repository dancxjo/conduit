import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdtempSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const workflow = readFileSync(new URL('../../.github/workflows/integration.yml', import.meta.url), 'utf8');
const adoption = workflow.slice(workflow.indexOf('\n  adoption:'), workflow.indexOf('\n  proof:'));
const script = adoption.match(/node --input-type=module <<'NODE'\n([\s\S]*?)\n          NODE/)[1]
  .split('\n').map(line => line.slice(10)).join('\n');
function select(t, inventory) {
  const directory = mkdtempSync(join(tmpdir(), 'adoption-history-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  writeFileSync(join(directory, 'adoption-inventory.json'), typeof inventory === 'string' ? inventory : JSON.stringify(inventory));
  const output = join(directory, 'outputs');
  const result = spawnSync(process.execPath, ['--input-type=module'], { input: script, encoding: 'utf8',
    env: { ...process.env, RUNNER_TEMP: directory, GITHUB_OUTPUT: output } });
  return { ...result, output: existsSync(output) ? readFileSync(output, 'utf8') : '' };
}

test('complete authenticated zero-run inventory needs only accepted source checkout', t => {
  const result = select(t, { total_count: 0, workflow_runs: [] });
  assert.equal(result.status, 0, result.stderr); assert.equal(result.output, 'depth=1\n');
});
for (const count of [1, 100, 101]) test(`any ${count} manual runs retain full reconstruction history`, t => {
  // Failed, foreign-branch or otherwise unusable metadata never selects shallow history.
  const result = select(t, { total_count: count,
    workflow_runs: Array.from({ length: Math.min(count, 100) }, () => ({ conclusion: 'failure', head_branch: 'other' })) });
  assert.equal(result.status, 0, result.stderr); assert.equal(result.output, 'depth=0\n');
});
for (const [name, inventory] of [
  ['invalid JSON', '{'], ['missing count', { workflow_runs: [] }],
  ['negative count', { total_count: -1, workflow_runs: [] }],
  ['fractional count', { total_count: 0.1, workflow_runs: [] }],
  ['string count', { total_count: '0', workflow_runs: [] }],
  ['missing inventory', { total_count: 0 }],
  ['incomplete zero inventory', { total_count: 0, workflow_runs: [{}] }],
  ['incomplete nonzero inventory', { total_count: 1, workflow_runs: [] }],
]) test(`malformed ${name} refuses without selecting a checkout depth`, t => {
  const result = select(t, inventory); assert.notEqual(result.status, 0); assert.equal(result.output, '');
});
test('depth selection changes neither train admission nor accepted-proof authority', () => {
  assert.match(adoption, /GH_TOKEN: \$\{\{ github.token \}\}/);
  assert.match(adoption, /head_sha=\$GITHUB_SHA&event=workflow_dispatch&per_page=100/);
  assert.match(adoption, /fetch-depth: \$\{\{ steps.history.outputs.depth \}\}/);
  assert.match(adoption, /ref: \$\{\{ github.sha \}\}/);
  assert.match(adoption, /persist-credentials: false/);
  assert.match(adoption, /node tools\/ci\/pipeline\/cli.mjs train-adoption "\$RUN_ID"/);
  assert.match(workflow.slice(0, workflow.indexOf('\n  adoption:')), /fetch-depth: 0/);
});
