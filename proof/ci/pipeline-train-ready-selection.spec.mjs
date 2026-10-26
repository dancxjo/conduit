import test from 'node:test';
import assert from 'node:assert/strict';
import { fixture } from './helpers/train-fixture.mjs';
import { selectTrainInputs } from '../../tools/ci/pipeline/train.mjs';
import { candidateAdmission, CandidateNotReadyError } from '../../tools/ci/pipeline/train-input-admission.mjs';
import { TARGETS } from '../../tools/ci/pipeline/targets.mjs';
import { UNIT_SHARDS } from '../../tools/ci/pipeline/plan.mjs';
import { observeCheapTrainPolicy } from '../../tools/ci/pipeline/train-cheap-policy.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';
import { discoverReadyTrainInputs } from '../../tools/ci/pipeline/train-ready-selection.mjs';

async function setup(t) {
  const f = await fixture(t);
  const repository = f.manifest.repository;
  const repositoryId = 11;
  for (const selected of f.selections) {
    for (const repo of [selected.pr.head.repo, selected.pr.base.repo,
      selected.candidateRun.repository, selected.candidateRun.head_repository]) repo.id = repositoryId;
  }
  const repo = { full_name: repository, id: repositoryId };
  const integration = { id: 1000, run_attempt: 1, repository: repo, head_repository: repo,
    head_sha: f.base, head_branch: 'dev', path: '.github/workflows/integration.yml',
    event: 'push', status: 'completed', conclusion: 'success' };
  const state = { dev: f.base, reads: new Map(), list: f.selections.map(value => ({ number: value.pr.number })),
    integration, selections: f.selections, incompleteIntegration: false, newerDevRun: false,
    missingCandidate: false, movedDev: false, changedAttempt: false, changedHead: false };
  const api = endpoint => {
    state.reads.set(endpoint, (state.reads.get(endpoint) ?? 0) + 1);
    if (endpoint.endsWith('/git/ref/heads/dev')) return { object: { sha: state.movedDev && state.reads.get(endpoint) > 1 ? '0'.repeat(40) : state.dev } };
    if (endpoint.includes('/workflows/integration.yml/runs?')) {
      const runs = [integration];
      if (state.newerDevRun) runs.push({ ...integration, id: 1001, status: 'in_progress', conclusion: null });
      return structuredClone({ total_count: runs.length + (state.incompleteIntegration ? 1 : 0), workflow_runs: runs });
    }
    if (endpoint.endsWith('/actions/runs/1000')) return structuredClone(integration);
    if (endpoint.endsWith('/actions/runs/1001')) return { ...integration, id: 1001, status: 'in_progress', conclusion: null };
    if (endpoint.includes('/pulls?')) return structuredClone(state.list);
    const pr = /\/pulls\/(\d+)$/.exec(endpoint);
    if (pr) {
      const value = structuredClone(state.selections.find(value => value.pr.number === Number(pr[1])).pr);
      if (state.changedHead && state.reads.get(endpoint) > 1) value.head.sha = '0'.repeat(40);
      return value;
    }
    const selected = state.selections.find(value => endpoint.includes(value.pr.head.sha) || endpoint.includes(`/runs/${value.candidateRun.id}`));
    if (!selected) throw new Error(`Unexpected API read ${endpoint}`);
    if (endpoint.includes('/workflows/candidate.yml/runs?')) return { total_count: state.missingCandidate ? 0 : 1,
      workflow_runs: state.missingCandidate ? [] : [structuredClone(selected.candidateRun)] };
    if (endpoint.endsWith(`/actions/runs/${selected.candidateRun.id}`)) {
      const value = structuredClone(selected.candidateRun);
      if (state.changedAttempt && state.reads.get(endpoint) > 1) value.run_attempt++;
      return value;
    }
    if (endpoint.includes('/jobs?')) return structuredClone(selected.candidateJobs);
    throw new Error(`Unexpected API read ${endpoint}`);
  };
  return { ...f, state, integration, api, observe: options => discoverReadyTrainInputs({
    policy: 'all-ready-same-repository-dev-prs', repository, repositoryId, api, ...options }) };
}

test('explicit ready policy returns unchanged authoritative selection from actual fixture', async t => {
  const f = await setup(t); const result = f.observe();
  assert.equal(result.kind, 'selected');
  assert.deepEqual(result.selection, selectTrainInputs({ repository: f.manifest.repository, base: f.base, selections: f.selections }));
  assert.deepEqual(result.classification, [{ number: 1, state: 'complete-candidate' }, { number: 2, state: 'complete-candidate' }]);
  assert.equal('source' in result.selection, false); assert.equal('digest' in result.selection, false);
});
for (const [name, mutate, expected] of [
  ['draft', selected => { selected.pr.draft = true; }, 'draft'],
  ['fork', selected => { selected.pr.head.repo = { full_name: 'fork/conduit', id: 12 }; }, 'fork'],
  ['conflicted', selected => { selected.pr.mergeable = false; }, 'conflicted'],
  ['closed after inventory', selected => { selected.pr.state = 'closed'; }, 'closed'],
  ['different base', selected => { selected.pr.base.ref = 'feature'; }, 'different-base'],
  ['running', selected => { selected.candidateRun.status = 'in_progress'; selected.candidateRun.conclusion = null; }, 'candidate-running'],
  ['cancelled', selected => { selected.candidateRun.conclusion = 'cancelled'; }, 'candidate-unready'],
]) test(`classifies ${name} without making it an input`, async t => {
  const f = await setup(t); mutate(f.selections[0]);
  const result = f.observe(); assert.equal(result.kind, 'selected');
  assert.deepEqual(result.selection.inputs.map(value => value.number), [2]);
  assert.deepEqual(result.classification[0], { number: 1, state: expected });
});
test('empty eligible inventory is idle; it manufactures no train', async t => {
  const f = await setup(t); for (const value of f.selections) value.pr.draft = true;
  const result = f.observe(); assert.equal(result.kind, 'idle'); assert.equal('selection' in result, false);
});
test('missing current Candidate is known unready, not historical green fallback', async t => {
  const f = await setup(t); f.state.missingCandidate = true;
  const result = f.observe(); assert.equal(result.kind, 'idle');
  assert.ok(result.classification.every(value => value.state === 'no-candidate'));
});
test('red dev blocks new work before pull request discovery', async t => {
  const f = await setup(t); f.integration.conclusion = 'failure';
  const result = f.observe(); assert.equal(result.kind, 'blocked');
  assert.equal([...f.state.reads.keys()].some(value => value.includes('/pulls')), false);
});
test('latest running dev run waits without selection over older green', async t => {
  const f = await setup(t); f.state.newerDevRun = true;
  const result = f.observe(); assert.equal(result.kind, 'waiting');
  assert.deepEqual(result.development, { source: f.base, runId: 1001, runAttempt: 1, status: 'in_progress' });
});
for (const [name, mutate] of [
  ['unknown mergeability', f => { f.selections[0].pr.mergeable = null; }],
  ['partial dev run inventory', f => { f.state.incompleteIntegration = true; }],
  ['100 open PR inventory ceiling', f => { f.state.list = Array.from({ length: 100 }, (_, i) => ({ number: i + 1 })); }],
  ['duplicate PR inventory', f => { f.state.list.push(f.state.list[0]); }],
  ['moved dev', f => { f.state.movedDev = true; }],
  ['moved head', f => { f.state.changedHead = true; }],
  ['stale Candidate attempt', f => { f.state.changedAttempt = true; }],
  ['wrong Candidate event', f => { f.selections[0].candidateRun.event = 'workflow_dispatch'; }],
  ['wrong Candidate repository ID', f => { f.selections[0].candidateRun.repository = { full_name: f.manifest.repository, id: 12 }; }],
  ['wrong job head', f => { f.selections[0].candidateJobs.jobs[0].head_sha = '0'.repeat(40); }],
  ['unknown Candidate job', f => { f.selections[0].candidateJobs.jobs[0].name = 'bypass'; }],
  ['missing Candidate jobs', f => { f.selections[0].candidateJobs.total_count++; }],
  ['unknown Candidate conclusion', f => { f.selections[0].candidateRun.conclusion = 'neutral'; }],
]) test(`refuses ${name}`, async t => {
  const f = await setup(t); mutate(f); assert.equal(f.observe().kind, 'refused');
});
test('ordinary failed preflight is classified unready while malformed failure refuses', async t => {
  const f = await setup(t); const selected = f.selections[0]; selected.candidateRun.conclusion = 'failure';
  selected.candidateJobs.jobs[0].conclusion = 'failure';
  for (const job of selected.candidateJobs.jobs) if (['proof / complete', 'candidate'].includes(job.name)) job.conclusion = 'failure';
  assert.equal(f.observe().classification[0].state, 'candidate-failed');
  selected.candidateJobs.jobs[1].name = 'unknown-hidden-check';
  assert.equal(f.observe().kind, 'refused');
});
test('adapter failures never disclose credentials or masquerade as idle', async t => {
  const f = await setup(t); const secret = 'private-observer-secret';
  const result = f.observe({ api: () => { throw new Error(secret); } });
  assert.equal(result.kind, 'refused'); assert.equal(JSON.stringify(result).includes(secret), false);
  assert.equal(f.observe({ policy: 'green-only' }).kind, 'refused');
});

function failed(selected, name) {
  selected.candidateRun.conclusion = 'failure';
  for (const job of selected.candidateJobs.jobs) {
    if ([name, 'proof / complete', 'candidate'].includes(job.name)) job.conclusion = 'failure';
  }
}
test('typed unready preserves rejection messages and never authorizes failed inputs', async t => {
  const f = await setup(t); const selected = f.selections[0]; failed(selected, 'proof / candidate/quick');
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), error =>
    error instanceof CandidateNotReadyError && error.message === 'Candidate admission: preflight must pass');
  selected.candidateJobs.jobs.splice(2, 1); selected.candidateJobs.total_count--;
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), error =>
    !(error instanceof CandidateNotReadyError));
  assert.equal(f.observe().kind, 'refused');
});
test('complete native-deferred admission remains ready through exact existing contract', async t => {
  const f = await setup(t); const selected = f.selections[0]; const run = selected.candidateRun;
  const names = [['proof / candidate/quick', 'success'], ['proof / admission', 'success'], ['proof / unit', 'skipped'],
    ...TARGETS.filter(value => value.family === 'conduitos').map(value =>
      [`proof / target / ${value.id}`, value.id === 'conduitos-x86_64' ? 'failure' : 'success']),
    ['proof / complete', 'failure'], ['candidate', 'failure']];
  run.conclusion = 'failure'; selected.candidateJobs = { total_count: names.length, jobs: names.map(([name, conclusion], i) => ({
    id: 30000 + i, run_id: run.id, run_attempt: run.run_attempt, head_sha: run.head_sha, status: 'completed', name, conclusion })) };
  const result = f.observe(); assert.equal(result.kind, 'selected');
  assert.equal(result.selection.inputs[0].admission.tier, 'native-deferred');
});
test('incomplete unit matrix stays refusal even when another required check actually failed', async t => {
  const f = await setup(t); const selected = f.selections[0]; failed(selected, 'proof / candidate/quick');
  const jobs = selected.candidateJobs.jobs; jobs.find(job => job.name === 'proof / unit').name = `proof / unit (${UNIT_SHARDS[0]})`;
  jobs.find(job => job.name.startsWith('proof / unit (')).conclusion = 'failure';
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), error => !(error instanceof CandidateNotReadyError));
  assert.equal(f.observe().kind, 'refused');
});
test('more than 32 eligible candidates refuses before Candidate observations', async t => {
  const f = await setup(t); const template = f.selections[0];
  f.state.selections = Array.from({ length: 33 }, (_, i) => ({ ...structuredClone(template), pr: { ...structuredClone(template.pr), number: i + 1 } }));
  f.state.list = f.state.selections.map(value => ({ number: value.pr.number }));
  assert.equal(f.observe().kind, 'refused');
  assert.equal([...f.state.reads.keys()].some(endpoint => endpoint.includes('/workflows/candidate')), false);
});
function cheap(selected, failure = false) {
  const run = selected.candidateRun; run.conclusion = failure ? 'failure' : 'success';
  const names = [['cheap-input/v1', failure ? 'failure' : 'success'], ['candidate', failure ? 'failure' : 'success'], ['proof', 'skipped']];
  selected.candidateJobs = { total_count: 3, jobs: names.map(([name, conclusion], i) => ({
    id: run.id * 100 + i, run_id: run.id, run_attempt: run.run_attempt, head_sha: run.head_sha,
    status: 'completed', name, conclusion })) };
}
function freshPolicy(repository) {
  const f = machinePolicyFixture(repository);
  const protection = { required_status_checks: { contexts: ['train-accepted'], checks: [{ context: 'train-accepted', app_id: 15368 }], strict: false },
    required_pull_request_reviews: null, enforce_admins: { enabled: true }, allow_force_pushes: { enabled: false }, allow_deletions: { enabled: false } };
  return () => observeCheapTrainPolicy({ config: f.config, repository,
    api: endpoint => endpoint === 'installation/repositories?per_page=100' ? f.repositories : f.machineRuleset,
    appApi: endpoint => endpoint === 'app' ? f.app : f.installation,
    policyApi: endpoint => {
      if (endpoint === 'user') return f.observerUser;
      if (endpoint.endsWith('/actions/variables/CONDUIT_PR_ADMISSION_TIER')) return { name: 'CONDUIT_PR_ADMISSION_TIER', value: 'cheap-input-v1' };
      if (endpoint.endsWith(`/rulesets/${f.config.rulesetId}`)) return f.ruleset;
      if (endpoint.endsWith('/rules/branches/dev?per_page=100')) return f.branchRules;
      if (endpoint.endsWith('/branches/dev/protection')) return protection;
      throw new Error('Unknown policy endpoint');
    } });
}
test('cheap selection requires a fresh opaque policy grant per input and refresh', async t => {
  const f = await setup(t); for (const selected of f.selections) cheap(selected);
  assert.equal(f.observe().kind, 'refused');
  assert.equal(f.observe({ observeCheapPolicy: () => ({ enabled: true }) }).kind, 'refused');
  let calls = 0; const grant = freshPolicy(f.manifest.repository);
  const observer = () => { calls++; return grant(); };
  const first = f.observe({ observeCheapPolicy: observer }); assert.equal(first.kind, 'selected'); assert.equal(calls, 2);
  assert.equal(f.observe({ observeCheapPolicy: observer }).kind, 'selected'); assert.equal(calls, 4);
  assert.equal(first.selection.inputs[0].admission.tier, 'cheap-input-v1');
});
test('typed cheap failure requires all three exact identities and actual failed required checks', async t => {
  const f = await setup(t); cheap(f.selections[0], true); const selected = f.selections[0];
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), error =>
    error instanceof CandidateNotReadyError && error.message === 'Candidate admission: cheap input requires successful complete job inventory');
  assert.equal(f.observe().classification[0].state, 'candidate-failed');
  selected.candidateJobs.jobs[2].name = 'unknown-hidden-proof';
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), error => !(error instanceof CandidateNotReadyError));
  assert.equal(f.observe().kind, 'refused');
});

test('actual complete unit failure is typed unready; wrapper-only failure remains refusal', async t => {
  const f = await setup(t); const selected = f.selections[0];
  failed(selected, 'proof / admission');
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), error =>
    error instanceof CandidateNotReadyError && error.message === 'Candidate admission: admission must pass');
  selected.candidateJobs.jobs.find(job => job.name === 'proof / admission').conclusion = 'success';
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), error => !(error instanceof CandidateNotReadyError));
  selected.candidateJobs.jobs = selected.candidateJobs.jobs.filter(job => job.name !== 'proof / unit');
  for (const [i, unit] of UNIT_SHARDS.entries()) selected.candidateJobs.jobs.push({
    ...selected.candidateJobs.jobs[0], id: 40000 + i, name: `proof / unit (${unit})`,
    conclusion: i === 0 ? 'failure' : 'success' });
  selected.candidateJobs.total_count = selected.candidateJobs.jobs.length;
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), error =>
    error instanceof CandidateNotReadyError && error.message === 'Candidate admission: selected portable unit matrix must completely pass');
  assert.equal(f.observe().classification[0].state, 'candidate-failed');
});


test('missing not-yet-emitted dev Integration waits with exact observed source', async t => {
  const f = await setup(t);
  const api = endpoint => endpoint.includes('/workflows/integration.yml/runs?')
    ? { total_count: 0, workflow_runs: [] } : f.api(endpoint);
  const result = f.observe({ api }); assert.equal(result.kind, 'waiting');
  assert.deepEqual(result.development, { source: f.base, runId: null, runAttempt: null, status: 'not-observed' });
  assert.equal([...f.state.reads.keys()].some(endpoint => endpoint.includes('/pulls')), false);
});
