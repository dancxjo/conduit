import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { beginProductCustody, recordProductCommand, finishProductCustody,
  abandonProductCustody, verifyProductCustody } from '../../tools/ci/pipeline/producer-custody.mjs';

const source = 'a'.repeat(40), tree = 'b'.repeat(40), target = 'hosted-linux';
async function setup(t) {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'producer-custody-'));
  t.after(() => { abandonProductCustody(); return rm(directory, { recursive: true, force: true }); });
  const acquisition = { target, sourceSha: source, outcome: 'success', tools: [{ name: 'rustc', version: 'fixture' }] };
  await mkdir(path.join(directory, 'target/acquisition'), { recursive: true });
  await writeFile(path.join(directory, 'target/acquisition', `${target}.json`), JSON.stringify(acquisition, null, 2) + '\n');
  const env = { GITHUB_ACTIONS: 'true', GITHUB_REPOSITORY: 'example/conduit', GITHUB_REPOSITORY_ID: '11',
    GITHUB_RUN_ID: '123', GITHUB_RUN_ATTEMPT: '2', GITHUB_SHA: source, GITHUB_WORKFLOW_SHA: source,
    GITHUB_JOB: 'target', GITHUB_WORKFLOW_REF: 'example/conduit/.github/workflows/integration.yml@refs/heads/train/fixture',
    ImageOS: 'ubuntu24', ImageVersion: 'fixture', PRIVATE_VALUE: 'must never be serialized', CONDUIT_TRAIN_MANIFEST_DIGEST: 'c'.repeat(64) };
  const expected = { source, tree, target, manifestDigest: 'c'.repeat(64), run: { id: 123, run_attempt: 2,
    repository: { full_name: 'example/conduit', id: 11 }, event: 'workflow_dispatch', head_branch: 'train/' + 'f'.repeat(64) } };
  env.GITHUB_WORKFLOW_REF = `example/conduit/.github/workflows/integration.yml@refs/heads/${expected.run.head_branch}`;
  const begin = (overrides = {}) => beginProductCustody({ source, target, directory, env,
    identity: program => program === 'git' ? tree : 'fixture tool version', ...overrides });
  return { directory, env, expected, begin };
}
test('producer binds observed run/attempt, tools, environment and selected command contract', async t => {
  const f = await setup(t); f.begin();
  recordProductCommand({ program: 'cargo', args: ['test', '--features', 'example'], durationMs: 1, outcome: 'success' });
  const result = finishProductCustody(f.directory);
  assert.equal(verifyProductCustody(result, f.expected), result);
  assert.equal(result.selection.policyTree, tree);
  assert.deepEqual(result.selection.commands[0].args, ['test', '--features', 'example']);
  const bytes = await readFile(path.join(f.directory, 'producer.json'), 'utf8');
  assert.ok(!bytes.includes('must never be serialized'));
  assert.throws(() => finishProductCustody(f.directory), /No active/);
});
test('old attempt, foreign workflow, missing inventory or altered acquisition cannot authorize reuse', async t => {
  const f = await setup(t); f.begin();
  recordProductCommand({ program: 'cargo', args: ['build'], durationMs: 1, outcome: 'success' });
  const result = finishProductCustody(f.directory);
  for (const change of [
    r => { r.run.attempt = 1; }, r => { r.run.repositoryId = 22; },
    r => { r.run.workflowSource = tree; }, r => { r.run.workflow = 'foreign'; },
    r => { r.selection.commands = []; }, r => { r.selection.policyTree = source; },
    r => { r.selection.commands[0].outcome = 'failed'; },
    r => { r.toolchain.acquisition.report.tools[0].version = 'changed'; },
    r => { r.environment.image.version = null; },
  ]) { const changed = structuredClone(result); change(changed); assert.throws(() => verifyProductCustody(changed, f.expected)); }
});
test('failure, empty command contract, incomplete run identity and absent acquisition refuse sealing', async t => {
  const f = await setup(t);
  assert.throws(() => f.begin({ env: { ...f.env, GITHUB_RUN_ATTEMPT: undefined } }), /Incomplete/);
  f.begin(); assert.throws(() => finishProductCustody(f.directory), /did not succeed/);
  f.begin(); recordProductCommand({ program: 'cargo', args: ['test'], durationMs: 1, outcome: 'failed' });
  assert.throws(() => finishProductCustody(f.directory), /did not succeed/);
  await rm(path.join(f.directory, 'target/acquisition', `${target}.json`));
  assert.throws(() => f.begin(), /lacks verified/);
});
