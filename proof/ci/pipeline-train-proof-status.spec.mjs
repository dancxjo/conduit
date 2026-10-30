import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { proofStatusIdentity, reportTrainProofStatus, verifyTrainProofStatus } from '../../tools/ci/pipeline/train-proof-status.mjs';
const repository = 'fixture/conduit', source = 'a'.repeat(40);
const run = { head_sha: source, id: 12, run_attempt: 3 };
const status = () => ({ id: 42, url: `https://api.github.com/repos/${repository}/statuses/${source}`,
  creator: { id: 41898282, login: 'github-actions[bot]', type: 'Bot' }, ...proofStatusIdentity(repository, source, 12, 3) });
test('workflow aggregate posts only current source, run and attempt using Actions authority', () => {
  let calls = 0;
  reportTrainProofStatus({ GITHUB_REPOSITORY: repository, GITHUB_SHA: source, GITHUB_RUN_ID: '12',
    GITHUB_RUN_ATTEMPT: '3', GITHUB_EVENT_NAME: 'workflow_dispatch', GITHUB_JOB: 'train-accepted' }, (program, args, options) => {
    calls++; assert.equal(program, 'gh'); assert.deepEqual(args, ['api', `repos/${repository}/statuses/${source}`, '--method', 'POST', '--input', '-']);
    assert.deepEqual(JSON.parse(options.input), proofStatusIdentity(repository, source, 12, 3));
    return { status: 0, stdout: JSON.stringify(status()) };
  }); assert.equal(calls, 1);
});
for (const [name, mutate] of [
  ['wrong source', s => { s.url += 'b'; }], ['wrong author', s => { s.creator.id++; }],
  ['writer author', s => { s.creator.login = 'conduit-integration[bot]'; }], ['human', s => { s.creator.type = 'User'; }],
  ['wrong run', s => { s.target_url = s.target_url.replace('/12/', '/13/'); }],
  ['wrong attempt', s => { s.description = 'Integration 12 attempt 2'; }],
  ['failed', s => { s.state = 'failure'; }], ['pending', s => { s.state = 'pending'; }],
  ['wrong context', s => { s.context = 'candidate'; }],
]) test(`proof status refuses ${name}`, () => { const s = status(); mutate(s); assert.throws(() => verifyTrainProofStatus([s], run, repository)); });
test('latest status cannot fall back to older success; absent or truncated inventory refuses', () => {
  const failed = { ...status(), state: 'failure' };
  for (const statuses of [[], [failed, status()], Array(100).fill(status())]) assert.throws(() => verifyTrainProofStatus(statuses, run, repository));
});
test('workflow requires successful aggregate before status and confines write permission to it', () => {
  const workflow = readFileSync('.github/workflows/integration.yml', 'utf8');
  const aggregate = workflow.slice(workflow.indexOf('\n  train-accepted:'));
  assert.ok(aggregate.includes('statuses: write'));
  assert.ok(aggregate.indexOf('test "$RESULT" = success') < aggregate.indexOf('run: node tools/ci/pipeline/train-proof-status.mjs'));
  const reporter = aggregate.slice(aggregate.indexOf('- name: Report independently'));
  assert.ok(!reporter.includes('always()'));
  assert.ok(!workflow.slice(0, workflow.indexOf('\n  train-accepted:')).includes('statuses: write'));
});
