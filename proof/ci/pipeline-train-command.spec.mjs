import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, writeFile, rm, symlink } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fixture } from './helpers/train-fixture.mjs';
import { createTrainCommands } from '../../tools/ci/pipeline/train-command.mjs';
import { sealTarget } from '../../tools/ci/pipeline/receipts.mjs';
import { TARGETS } from '../../tools/ci/pipeline/targets.mjs';

async function commands(t) {
  const f = await fixture(t);
  const output = await mkdtemp(path.join(os.tmpdir(), 'conduit-train-output-'));
  t.after(() => rm(output, { recursive: true, force: true }));
  const calls = [];
  const runs = f.selections.map((selection, i) => [{ ...selection.candidateRun, id: i + 10 }]);
  const command = (program, args, options) => {
    if (program === 'git') return spawnSync(program, args, options);
    assert.equal(program, 'gh'); assert.equal(args[0], 'api'); assert.equal(args.length, 2);
    calls.push(args[1]);
    let value;
    if (args[1].endsWith('/git/ref/heads/dev')) value = { object: { sha: f.base } };
    else if (args[1].endsWith('/actions/runs/123')) value = f.observation.run;
    else if (args[1].includes('/pulls/')) value = f.selections.find(s => args[1].endsWith(`/pulls/${s.pr.number}`))?.pr;
    else if (args[1].includes('/actions/workflows/candidate.yml/runs?')) {
      const index = f.selections.findIndex(s => args[1].includes(`head_sha=${s.pr.head.sha}`));
      value = { workflow_runs: runs[index] };
    } else assert.fail(`Unexpected API ${args[1]}`);
    return { status: 0, stdout: JSON.stringify(value) };
  };
  const api = createTrainCommands({ command, directory: f.directory });
  return { ...f, output, calls, runs, api, filename: path.join(output, 'train.json') };
}

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

test('supported pipeline CLI exposes both train entrances and refuses missing arguments', () => {
  for (const name of ['train-capture', 'train-verify']) {
    const result = spawnSync('node', ['tools/ci/pipeline/cli.mjs', name], { encoding: 'utf8' });
    assert.equal(result.status, 1);
    assert.match(result.stderr, new RegExp(`usage: ${name}`));
  }
});
