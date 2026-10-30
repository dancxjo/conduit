import test from 'node:test';
import assert from 'node:assert/strict';
import { writeFile, readFile } from 'node:fs/promises';
import path from 'node:path';
import { commands } from './helpers/train-command-fixture.mjs';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';
import { observeCheapTrainPolicy } from '../../tools/ci/pipeline/train-cheap-policy.mjs';
import { createTrainCommands } from '../../tools/ci/pipeline/train-command.mjs';
import { createTrainDispatcher } from '../../tools/ci/pipeline/train-dispatch.mjs';
import { createTrainLander } from '../../tools/ci/pipeline/train-land.mjs';
import { createTrainProofAdmission } from '../../tools/ci/pipeline/train-proof-admission.mjs';

async function fixture(t) {
  const f = await commands(t);
  const selected = f.selections[0], run = selected.candidateRun;
  const states = [['cheap-input/v1', 'success'], ['proof', 'skipped'], ['candidate', 'success']];
  selected.candidateJobs = { total_count: 3, jobs: states.map(([name, conclusion], i) => ({
    id: 10000 + i, run_id: run.id, run_attempt: run.run_attempt, head_sha: run.head_sha,
    status: 'completed', name, conclusion,
  })) };
  const machine = machinePolicyFixture('example/conduit'); let observations = 0;
  const protection = { required_status_checks: { strict: false, contexts: ['train-proof'],
    checks: [{ context: 'train-proof', app_id: 15368 }] }, required_pull_request_reviews: null,
    enforce_admins: { enabled: true }, allow_force_pushes: { enabled: false }, allow_deletions: { enabled: false } };
  const observeCheapPolicy = () => {
    observations++;
    return observeCheapTrainPolicy({ config: machine.config, repository: machine.repository,
      appApi: endpoint => endpoint === 'app' ? machine.app : machine.installation,
      api: endpoint => endpoint.startsWith('installation/') ? machine.repositories : machine.machineRuleset,
      policyApi: endpoint => {
        if (endpoint === 'user') return machine.observerUser;
        if (endpoint.endsWith('/protection')) return protection;
        if (endpoint.includes('/actions/variables/')) return { name: 'CONDUIT_PR_ADMISSION_TIER', value: 'cheap-input-v1' };
        if (endpoint.includes('/rulesets/')) return machine.ruleset;
        if (endpoint.includes('/rules/branches/')) return machine.branchRules;
        assert.fail(endpoint);
      } });
  };
  const trusted = createTrainCommands({ command: f.command, directory: f.directory, observeCheapPolicy });
  const manifest = await trusted.capture('example/conduit', f.source, [1, 2], f.filename);
  f.observation.run.head_branch = manifest.branch;
  return { ...f, manifest, trusted, observeCheapPolicy, observations: () => observations };
}

test('read-only admission reconstructs real combined cheap Git tree without privileged grants', async t => {
  const f = await fixture(t); const before = f.observations(), endpoints = [];
  const admit = createTrainProofAdmission({ directory: f.directory, command(program, args, options) {
    assert.equal(program, 'gh'); assert.equal(args[0], 'api'); assert.equal(args.length, 2);
    assert.doesNotMatch(args[1], /variables|protection|rulesets|installation|\/app/);
    endpoints.push(args[1]); return f.command(program, args, options);
  } });
  assert.deepEqual(await admit(f.filename), { source: f.source, tree: f.manifest.tree, digest: f.manifest.digest });
  assert.equal(f.observations(), before);
  assert.equal(endpoints.filter(value => value.endsWith('/pulls/1')).length, 2);
  assert.deepEqual(JSON.parse(await readFile(f.filename, 'utf8')), f.manifest);
});

for (const [name, mutate] of [
  ['PR head', f => { f.selections[0].pr.head.sha = 'a'.repeat(40); }],
  ['PR readiness', f => { f.selections[0].pr.draft = true; }],
  ['Candidate attempt', f => { f.runs[0][0].run_attempt++; }],
  ['Candidate run', f => { f.runs[0][0].id++; }],
  ['job identity', f => { f.selections[0].candidateJobs.jobs[0].id++; }],
  ['quick result', f => { f.selections[0].candidateJobs.jobs[0].conclusion = 'failure'; }],
  ['job inventory', f => { f.selections[0].candidateJobs.total_count++; }],
]) test(`read-only admission refuses ${name} changed after first reconstruction`, async t => {
  const f = await fixture(t); let devReads = 0;
  const admit = createTrainProofAdmission({ directory: f.directory, command(program, args, options) {
    if (args[1].endsWith('/git/ref/heads/dev') && ++devReads === 2) mutate(f);
    return f.command(program, args, options);
  } });
  await assert.rejects(admit(f.filename));
  assert.equal(devReads, 2);
});

test('read-only admission refuses moved dev, altered source tree, and dirty checkout', async t => {
  const f = await fixture(t);
  const moved = createTrainProofAdmission({ directory: f.directory, command(program, args, options) {
    if (args[1].endsWith('/git/ref/heads/dev')) return { status: 0, stdout: JSON.stringify({ object: { sha: 'a'.repeat(40) } }) };
    return f.command(program, args, options);
  } });
  await assert.rejects(moved(f.filename), /Development advanced/);
  let reads = 0;
  const movedAfter = createTrainProofAdmission({ directory: f.directory, command(program, args, options) {
    if (args[1].endsWith('/git/ref/heads/dev') && ++reads === 2) return { status: 0, stdout: JSON.stringify({ object: { sha: 'a'.repeat(40) } }) };
    return f.command(program, args, options);
  } });
  await assert.rejects(movedAfter(f.filename), /Development advanced/);
  assert.equal(reads, 2);
  const { createHash } = await import('node:crypto');
  const altered = { ...f.manifest, tree: 'a'.repeat(40) };
  const payload = { ...altered }; delete payload.digest;
  altered.digest = createHash('sha256').update(JSON.stringify(payload)).digest('hex');
  const alteredFile = path.join(f.output, 'altered-tree.json'); await writeFile(alteredFile, JSON.stringify(altered));
  await assert.rejects(createTrainProofAdmission({ command: f.command, directory: f.directory })(alteredFile), /source tree differs/);
  await writeFile(path.join(f.directory, 'unreviewed'), 'dirty');
  await assert.rejects(createTrainProofAdmission({ command: f.command, directory: f.directory })(f.filename), /untracked/);
});

test('unprivileged proof identity cannot authorize capture, dispatch, or landing', async t => {
  const f = await fixture(t);
  await createTrainProofAdmission({ command: f.command, directory: f.directory })(f.filename);
  await assert.rejects(f.api.capture('example/conduit', f.source, [1, 2], path.join(f.output, 'unauthorized.json')), /external cheap policy observer/);
  await assert.rejects(createTrainDispatcher({ command: f.command, directory: f.directory })(f.filename), /external cheap policy observer/);
  await assert.rejects(f.api.acquire(f.filename, 123, path.join(f.output, 'acquired')), /external cheap policy observer/);
  await assert.rejects(f.api.restore(f.filename, 123, path.join(f.output, 'acquisition.json'), path.join(f.output, 'restored')), /external cheap policy observer/);
  await assert.rejects(f.api.verify(f.filename, 123, path.join(f.output, 'products')), /external cheap policy observer/);
  const evidence = path.join(f.output, 'landing');
  await assert.rejects(createTrainLander({ command: f.command, directory: f.directory })(f.filename, 123,
    path.join(f.output, 'archives'), evidence), /external cheap policy observer/);
  await assert.rejects(readFile(path.join(evidence, 'landing-intent.json')), /ENOENT/);
});

test('dispatcher preserves trusted observer or command injection for cheap selection', async t => {
  const f = await fixture(t);
  const command = (program, args, options) => {
    if (args[1]?.includes('/git/matching-refs/')) return { status: 0, stdout: JSON.stringify([
      { ref: `refs/heads/${f.manifest.branch}`, object: { sha: f.source } }]) };
    if (args[1]?.includes('/integration.yml/runs?')) return { status: 0,
      stdout: JSON.stringify({ workflow_runs: [f.observation.run] }) };
    return f.command(program, args, options);
  };
  const before = f.observations();
  assert.equal((await createTrainDispatcher({ command, directory: f.directory,
    observeCheapPolicy: f.observeCheapPolicy })(f.filename)).reused, true);
  assert.equal(f.observations(), before + 1);
  assert.equal((await createTrainDispatcher({ command, directory: f.directory, commands: f.trusted })(f.filename)).reused, true);
  assert.equal(f.observations(), before + 2);
});

test('CLI train-admit has only the read-only proof scheduling role', async () => {
  const cli = await readFile('tools/ci/pipeline/cli.mjs', 'utf8');
  const body = cli.split("case 'train-admit':")[1].split("case 'train-land':")[0];
  assert.match(body, /createTrainProofAdmission/);
  assert.doesNotMatch(body, /prepareDispatch|createTrainCommands|observeCheapPolicy|AppSession/);
});
