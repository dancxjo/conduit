import assert from 'node:assert/strict';
import test from 'node:test';
import { candidateAdmission } from '../../tools/ci/pipeline/train-input-admission.mjs';
import { verifyTrainRun } from '../../tools/ci/pipeline/train.mjs';
import { TARGETS } from '../../tools/ci/pipeline/targets.mjs';
import { fixture } from './helpers/train-fixture.mjs';

function nativeFailure(selection) {
  selection.candidateRun.conclusion = 'failure';
  const run = selection.candidateRun;
  const states = [['proof / candidate/quick', 'success'], ['proof / admission', 'success'],
    ['proof / unit', 'skipped'], ...TARGETS.filter(t => t.family === 'conduitos').map(t =>
      [`proof / target / ${t.id}`, t.id === 'conduitos-x86_64' ? 'failure' : 'success']),
    ['proof / complete', 'failure'], ['candidate', 'failure']];
  selection.candidateJobs = { total_count: states.length, jobs: states.map(([name, conclusion], i) => ({
    id: 10000 + i, run_id: run.id, run_attempt: run.run_attempt, head_sha: run.head_sha,
    name, conclusion, status: 'completed',
  })) };
  return selection;
}

test('bounded native failure admits only train inputs and preserves every negative job', async t => {
  const f = await fixture(t); const selected = nativeFailure(f.selections[0]);
  const manifest = f.capture(); f.observation.run.head_branch = manifest.branch;
  const admission = manifest.inputs[0].admission;
  assert.equal(manifest.schema, 'conduit.integration-train/v2');
  assert.equal(admission.tier, 'native-deferred');
  assert.equal(admission.conclusion, 'failure');
  assert.equal(admission.runId, selected.candidateRun.id);
  assert.equal(admission.runAttempt, 1);
  assert.equal(admission.jobs.length, 10);
  assert.deepEqual(admission.deferredNativeProducts, ['conduitos-x86_64']);
  assert.equal(verifyTrainRun(manifest, f.observation).integrationRun, 123);
  const failedCombined = structuredClone(f.observation); failedCombined.run.conclusion = 'failure';
  assert.throws(() => verifyTrainRun(manifest, failedCombined), /successful exhaustive Integration/);
  selected.candidateJobs.jobs.reverse(); assert.deepEqual(f.capture(), manifest);
});

for (const [name, mutate] of [
  ['missing admission', s => { s.candidateJobs.jobs = s.candidateJobs.jobs.filter(j => j.name !== 'proof / admission'); s.candidateJobs.total_count--; }],
  ['failed preflight', s => { s.candidateJobs.jobs[0].conclusion = 'failure'; }],
  ['failed admission', s => { s.candidateJobs.jobs[1].conclusion = 'failure'; }],
  ['cancelled native job', s => { s.candidateJobs.jobs[3].conclusion = 'cancelled'; }],
  ['running job', s => { s.candidateJobs.jobs[3].status = 'in_progress'; }],
  ['missing native matrix member', s => { s.candidateJobs.jobs.splice(3, 1); s.candidateJobs.total_count--; }],
  ['truncated jobs', s => { s.candidateJobs.total_count++; }],
  ['unknown job', s => { s.candidateJobs.jobs[2].name = 'proof / hidden bypass'; }],
  ['duplicate job name', s => { s.candidateJobs.jobs[2].name = s.candidateJobs.jobs[0].name; }],
  ['duplicate job id', s => { s.candidateJobs.jobs[2].id = s.candidateJobs.jobs[0].id; }],
  ['wrong attempt', s => { s.candidateJobs.jobs[3].run_attempt++; }],
  ['wrong run', s => { s.candidateJobs.jobs[3].run_id++; }],
  ['wrong head', s => { s.candidateJobs.jobs[3].head_sha = 'a'.repeat(40); }],
  ['missing selected units', s => { s.candidateJobs.jobs[2].name = 'proof / unit (foundation)'; s.candidateJobs.jobs[2].conclusion = 'success'; }],
  ['skipped admission', s => { s.candidateJobs.jobs[1].conclusion = 'skipped'; }],
  ['contradictory wrapper', s => { s.candidateJobs.jobs.at(-1).conclusion = 'success'; }],
]) test(`native-deferred tier refuses ${name}`, async t => {
  const f = await fixture(t); const selected = nativeFailure(f.selections[0]); mutate(selected);
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), /Candidate admission/);
});

test('selected portable or host failures cannot be deferred with native product failure', async t => {
  const f = await fixture(t); const selected = nativeFailure(f.selections[0]);
  for (const target of TARGETS.filter(t => t.family === 'hosted')) {
    selected.candidateJobs.jobs.push({ ...selected.candidateJobs.jobs[0], id: 20000 + selected.candidateJobs.jobs.length,
      name: `proof / target / ${target.id}`, conclusion: target.id === 'hosted-linux' ? 'failure' : 'success' });
  }
  selected.candidateJobs.total_count = selected.candidateJobs.jobs.length;
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), /host or non-native product/);
});

test('run, attempt and exact job changes invalidate an already captured train', async t => {
  const f = await fixture(t); nativeFailure(f.selections[0]); const manifest = f.capture();
  for (const mutate of [
    s => { s.candidateRun.id++; for (const job of s.candidateJobs.jobs) job.run_id++; },
    s => { s.candidateRun.run_attempt++; for (const job of s.candidateJobs.jobs) job.run_attempt++; },
    s => { s.candidateJobs.jobs[0].id += 100000; },
  ]) {
    const observation = structuredClone(f.observation); mutate(observation.selections[0]);
    assert.throws(() => verifyTrainRun(manifest, observation), /admission\/run\/attempt\/job evidence changed/);
  }
});

test('failed selected portable unit matrix refuses even with deferable native failure', async t => {
  const { UNIT_SHARDS } = await import('../../tools/ci/pipeline/plan.mjs');
  const f = await fixture(t); const selected = nativeFailure(f.selections[0]);
  selected.candidateJobs.jobs = selected.candidateJobs.jobs.filter(j => j.name !== 'proof / unit');
  for (const [i, unit] of UNIT_SHARDS.entries()) selected.candidateJobs.jobs.push({
    ...selected.candidateJobs.jobs[0], id: 30000 + i, name: `proof / unit (${unit})`,
    conclusion: unit === 'foundation' ? 'failure' : 'success',
  });
  selected.candidateJobs.total_count = selected.candidateJobs.jobs.length;
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), /portable unit matrix/);
});

test('literal unexpanded target placeholder is accepted only as the sole skipped target', async t => {
  const f = await fixture(t); const selected = f.selections[0];
  assert.equal(candidateAdmission(selected.candidateRun, selected.candidateJobs).tier, 'complete-candidate');
  nativeFailure(selected);
  selected.candidateJobs.jobs.push({ ...selected.candidateJobs.jobs[0], id: 40000,
    name: 'proof / target / ${{ matrix.id }}', conclusion: 'skipped' });
  selected.candidateJobs.total_count++;
  assert.throws(() => candidateAdmission(selected.candidateRun, selected.candidateJobs), /ambiguous target selection/);
});
