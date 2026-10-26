import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdir, readFile, writeFile, symlink } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { commands } from './helpers/train-command-fixture.mjs';
import { createTrainCommands } from '../../tools/ci/pipeline/train-command.mjs';
import { sealTarget } from '../../tools/ci/pipeline/receipts.mjs';
import { TARGETS } from '../../tools/ci/pipeline/targets.mjs';

test('live adapter captures exact Git source and only performs API reads', async t => {
  const f = await commands(t);
  const result = await f.api.capture('example/conduit', f.source, ['2', '1'], f.filename);
  assert.deepEqual(JSON.parse(await readFile(f.filename, 'utf8')), result);
  assert.deepEqual(result, f.manifest);
  assert.equal(f.calls.filter(p => p.endsWith('/git/ref/heads/dev')).length, 2);
  await assert.rejects(f.api.capture('example/conduit', f.source, [1, 2], f.filename), /EEXIST/);
});

test('latest failed or running attempt cannot be replaced with older green evidence', async t => {
  const f = await commands(t);
  for (const state of [{ conclusion: 'failure' }, { status: 'in_progress', conclusion: null },
    { conclusion: 'failure', pull_requests: [] }]) {
    f.runs[0] = [{ ...f.selections[0].candidateRun, id: 9 },
      { ...f.selections[0].candidateRun, id: 10, ...state }];
    await assert.rejects(f.api.capture('example/conduit', f.source, [1, 2], f.filename), /Candidate/);
  }
});

test('dirty source and duplicate identities refuse before creating a manifest', async t => {
  const f = await commands(t);
  await assert.rejects(f.api.capture('example/conduit', f.source, [1, 1], f.filename), /Duplicate/);
  await writeFile(path.join(f.directory, 'untracked'), 'unreviewed change');
  await assert.rejects(f.api.capture('example/conduit', f.source, [1, 2], f.filename), /untracked/);
  await assert.rejects(readFile(f.filename), /ENOENT/);
});

test('live verification rechecks exact source tree and existing full product receipts', async t => {
  const f = await commands(t);
  await f.api.capture('example/conduit', f.source, [1, 2], f.filename);
  const bundle = path.join(f.output, 'products'); await mkdir(bundle);
  for (const target of TARGETS) {
    const directory = path.join(bundle, target.id); await mkdir(directory);
    await writeFile(path.join(directory, 'fixture'), 'inert fixture');
    await sealTarget({ directory, target: target.id, sha: f.source, proofClass: target.proofClass });
  }
  assert.equal((await f.api.verify(f.filename, 123, bundle)).source, f.source);
  const changed = { ...f.manifest, tree: f.base };
  const payload = { ...changed }; delete payload.digest;
  changed.digest = createHash('sha256').update(JSON.stringify(payload)).digest('hex');
  await writeFile(f.filename, JSON.stringify(changed));
  await assert.rejects(f.api.verify(f.filename, 123, bundle), /tree mismatch/);
});

test('symlink and oversized manifest files refuse before network observation', async t => {
  const f = await commands(t);
  await writeFile(f.filename, ' '.repeat(65537));
  await assert.rejects(f.api.verify(f.filename, 123, f.output), /manifest file/);
  await symlink(f.filename, path.join(f.output, 'link'));
  await assert.rejects(f.api.verify(path.join(f.output, 'link'), 123, f.output), /manifest file/);
  assert.equal(f.calls.length, 0);
});

test('supported pipeline CLI exposes train entrances and refuses missing arguments', () => {
  for (const name of ['train-assemble', 'train-capture', 'train-dispatch', 'train-admit', 'train-adoption', 'train-acquire', 'train-restore', 'train-land', 'train-verify',
    'compiler-cache-engine', 'compiler-cache-prepare', 'compiler-cache-build', 'compiler-cache-clear', 'compiler-cache-proof']) {
    const result = spawnSync('node', ['tools/ci/pipeline/cli.mjs', name], { encoding: 'utf8' });
    assert.equal(result.status, 1);
    assert.match(result.stderr, new RegExp(`usage: ${name}`));
  }
});

test('supported acquisition rechecks live selection and obtains the full run-owned archive inventory', async t => {
  const f = await commands(t);
  f.observation.run.run_attempt = 1;
  f.observation.run.repository.id = 11;
  f.observation.run.head_repository.id = 11;
  await f.api.capture('example/conduit', f.source, [1, 2], f.filename);
  const bytes = Buffer.from('inert archive fixture');
  const artifacts = TARGETS.map((target, i) => ({ name: `product-${target.id}`, id: i + 100,
    size_in_bytes: bytes.length, expired: false,
    digest: `sha256:${createHash('sha256').update(bytes).digest('hex')}`,
    workflow_run: { id: 123, repository_id: 11, head_repository_id: 11, head_sha: f.source } }));
  const artifactCommand = async (program, args, options) => {
    assert.equal(program, 'gh');
    if (args[1].endsWith('/zip')) {
      await options.output.write(bytes); return { status: 0 };
    }
    if (args[1].includes('/artifacts?')) {
      return { status: 0, stdout: JSON.stringify({ total_count: artifacts.length, artifacts }) };
    }
    return { status: 0, stdout: JSON.stringify(f.observation.run) };
  };
  const api = createTrainCommands({ command: f.command, directory: f.directory, artifactCommand });
  const receipt = await api.acquire(f.filename, 123, path.join(f.output, 'acquired'));
  assert.equal(receipt.artifacts.length, TARGETS.length);
  assert.equal(receipt.source, f.source);
  assert.equal(receipt.runAttempt, 1);
  assert.equal(f.git('status', '--porcelain'), '');
});

test('machine assembly preserves exact inputs and original worktree, then captures the combined tree', async t => {
  const f = await commands(t);
  f.git('remote', 'add', 'origin', f.directory);
  const worktree = path.join(f.output, 'assembled');
  const manifest = await f.api.assemble('example/conduit', [2, 1], worktree, f.filename);
  assert.equal(manifest.tree, f.manifest.tree);
  assert.equal(manifest.branch, f.manifest.branch);
  assert.equal(f.git('rev-parse', 'HEAD'), f.source);
  assert.equal(f.git('status', '--porcelain'), '');
  for (const input of f.selections) f.git('merge-base', '--is-ancestor', input.pr.head.sha, manifest.source);
  assert.deepEqual(JSON.parse(await readFile(f.filename, 'utf8')), manifest);
  await assert.rejects(f.api.assemble('example/conduit', [1, 2], worktree, f.filename), /already exists/);
});

test('assembly refuses failed inputs before fetching or creating a worktree', async t => {
  const f = await commands(t);
  f.runs[0][0].conclusion = 'failure';
  const worktree = path.join(f.output, 'assembled');
  await assert.rejects(f.api.assemble('example/conduit', [1, 2], worktree, f.filename), /Candidate/);
  await assert.rejects(readFile(f.filename), /ENOENT/);
  assert.equal(f.git('branch', '--list', 'train/*'), '');
  assert.equal(f.git('rev-parse', 'HEAD'), f.source);
});

test('a conflicting assembly retains its branch and conflicted worktree without claiming a manifest', async t => {
  const f = await commands(t);
  for (const [index, branch] of ['conflict-first', 'conflict-second'].entries()) {
    f.git('checkout', '-b', branch, f.base);
    await writeFile(path.join(f.directory, 'base'), branch);
    f.git('add', 'base'); f.git('commit', '-m', branch);
    const head = f.git('rev-parse', 'HEAD');
    f.selections[index].pr.head.sha = head;
    f.runs[index][0].head_sha = head;
  }
  f.git('remote', 'add', 'origin', f.directory);
  const worktree = path.join(f.output, 'conflicted');
  await assert.rejects(f.api.assemble('example/conduit', [1, 2], worktree, f.filename), /retained train\//);
  assert.match(f.git('-C', worktree, 'status', '--porcelain'), /UU base/);
  await assert.rejects(readFile(f.filename), /ENOENT/);
});

test('development changing after assembly preserves the candidate but refuses a manifest', async t => {
  const f = await commands(t);
  f.git('remote', 'add', 'origin', f.directory);
  let observations = 0;
  const api = createTrainCommands({ directory: f.directory, command(program, args, options) {
    const result = f.command(program, args, options);
    if (program === 'gh' && args[1].endsWith('/git/ref/heads/dev') && ++observations === 3) {
      return { status: 0, stdout: JSON.stringify({ object: { sha: f.source } }) };
    }
    return result;
  } });
  const worktree = path.join(f.output, 'stale');
  await assert.rejects(api.assemble('example/conduit', [1, 2], worktree, f.filename), /retained train\//);
  assert.equal(f.git('-C', worktree, 'status', '--porcelain'), '');
  await assert.rejects(readFile(f.filename), /ENOENT/);
});

test('attempt-specific jobs are complete and a rerun during observation refuses', async t => {
  const f = await commands(t); let observations = 0;
  const api = createTrainCommands({ directory: f.directory, command(program, args, options) {
    const result = f.command(program, args, options);
    if (program === 'gh' && args[1].endsWith('/actions/runs/10') && ++observations === 2) {
      const body = JSON.parse(result.stdout); body.run_attempt = 2;
      return { ...result, stdout: JSON.stringify(body) };
    }
    return result;
  } });
  await assert.rejects(api.capture('example/conduit', f.source, [1, 2], f.filename), /attempt changed/);
  assert.ok(f.calls.some(p => p.endsWith('/actions/runs/10/attempts/1/jobs?per_page=100')));
  await assert.rejects(readFile(f.filename), /ENOENT/);
});

test('truncated attempt jobs and a newly started Candidate cannot use old admission', async t => {
  for (const mode of ['truncated', 'new-run']) {
    const f = await commands(t); let inventories = 0;
    const api = createTrainCommands({ directory: f.directory, command(program, args, options) {
      const result = f.command(program, args, options);
      if (program !== 'gh') return result;
      const body = JSON.parse(result.stdout);
      if (mode === 'truncated' && args[1].includes('/attempts/')) body.total_count++;
      if (mode === 'new-run' && args[1].includes('/workflows/candidate.yml/runs?') && ++inventories === 2) {
        body.workflow_runs.push({ ...body.workflow_runs[0], id: 99, status: 'in_progress', conclusion: null }); body.total_count++;
      }
      return { ...result, stdout: JSON.stringify(body) };
    } });
    await assert.rejects(api.capture('example/conduit', f.source, [1, 2], f.filename), /Candidate.*inventory|Candidate changed/);
    await assert.rejects(readFile(f.filename), /ENOENT/);
  }
});

test('oversized complete admission receipts refuse before any assembly Git mutation', async t => {
  const { UNIT_SHARDS } = await import('../../tools/ci/pipeline/plan.mjs');
  const f = await commands(t); const selections = Array.from({ length: 32 }, (_, i) => {
    const pr = structuredClone(f.selections[0].pr); pr.number = i + 1; pr.head.sha = (i + 1).toString(16).padStart(40, '0');
    const run = { ...f.selections[0].candidateRun, id: i + 100, head_sha: pr.head.sha, pull_requests: [{ number: pr.number }] };
    const names = ['proof / candidate/quick', 'proof / admission', 'proof / complete', 'candidate',
      ...UNIT_SHARDS.map(unit => `proof / unit (${unit})`), ...TARGETS.map(target => `proof / target / ${target.id}`)];
    const jobs = { total_count: names.length, jobs: names.map((name, j) => ({ id: 100000 + i * 100 + j,
      run_id: run.id, run_attempt: 1, head_sha: pr.head.sha, name, status: 'completed', conclusion: 'success' })) };
    return { pr, run, jobs };
  });
  let gitCalls = 0;
  const api = createTrainCommands({ directory: f.directory, command(program, args) {
    if (program === 'git') { gitCalls++; throw new Error('Assembly must not reach Git'); }
    const endpoint = args[1]; let body;
    if (endpoint.endsWith('/git/ref/heads/dev')) body = { object: { sha: f.base } };
    else if (/\/pulls\/\d+$/.test(endpoint)) body = selections.find(s => endpoint.endsWith(`/pulls/${s.pr.number}`)).pr;
    else if (endpoint.includes('/workflows/candidate.yml/runs?')) body = { total_count: 1,
      workflow_runs: [selections.find(s => endpoint.includes(`head_sha=${s.pr.head.sha}`)).run] };
    else if (endpoint.includes('/attempts/')) body = selections.find(s => endpoint.includes(`/runs/${s.run.id}/`)).jobs;
    else body = selections.find(s => endpoint.endsWith(`/runs/${s.run.id}`)).run;
    return { status: 0, stdout: JSON.stringify(body) };
  } });
  await assert.rejects(api.assemble('example/conduit', selections.map(s => s.pr.number), path.join(f.output, 'oversized'), f.filename), /manifest exceeds bound/);
  assert.equal(gitCalls, 0);
  await assert.rejects(readFile(f.filename), /ENOENT/);
});
