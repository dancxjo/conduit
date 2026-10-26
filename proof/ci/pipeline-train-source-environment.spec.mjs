import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertSourceCheckout } from '../../tools/ci/pipeline/source.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
test('real Git assertions and reconstruction receive explicit environment including synthetic commit identity', async t => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'train-source-env-'));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const realGit = spawnSync('which', ['git'], { encoding: 'utf8' }).stdout.trim();
  const log = path.join(dir, 'observations');
  await writeFile(path.join(dir, 'git'), `#!${process.execPath}\nconst fs=require('node:fs'),cp=require('node:child_process');\nconst args=process.argv.slice(2);fs.appendFileSync(${JSON.stringify(log)},JSON.stringify({safe:process.env.FIXTURE_SAFE==='yes',owner:Object.hasOwn(process.env,'FIXTURE_OWNER'),commit:args.includes('commit-tree'),author:process.env.GIT_AUTHOR_NAME==='Conduit train'})+'\\n');\nconst r=cp.spawnSync(${JSON.stringify(realGit)},args,{env:process.env,stdio:'inherit'});process.exit(r.status??1);\n`, { mode: 0o700 });
  const script = `import {commands} from './proof/ci/helpers/train-command-fixture.mjs';
import {createTrainCommands} from './tools/ci/pipeline/train-command.mjs';
const cleanup=[];const f=await commands({after:fn=>cleanup.push(fn)});
try { const safe={PATH:${JSON.stringify(`${dir}:${process.env.PATH}`)},FIXTURE_SAFE:'yes'};
const api=createTrainCommands({command:f.command,directory:f.directory,sourceEnvironment:safe});
await api.capture(f.manifest.repository,f.manifest.source,f.manifest.inputs.map(p=>p.number),f.filename);
await api.prepareDispatch(f.filename);
}finally{for(const fn of cleanup.reverse())await fn();}`;
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', script], {
    cwd: root, env: { ...process.env, FIXTURE_OWNER: 'fixture-only-owner' }, encoding: 'utf8' });
  assert.ok(result.status === 0, 'fixture capture succeeds');
  const observations = (await readFile(log, 'utf8')).trim().split('\n').map(JSON.parse);
  assert.ok(observations.length > 5);
  assert.ok(observations.every(value => value.safe && !value.owner));
  assert.ok(observations.some(value => value.commit && value.author));
});

test('privileged source failures hide child diagnostics while default remains useful', async t => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'train-source-fail-'));
  t.after(() => rm(dir, { recursive: true, force: true }));
  await writeFile(path.join(dir, 'git'), '#!/bin/sh\necho fixture-sensitive-diagnostic >&2\nexit 1\n', { mode: 0o700 });
  assert.throws(() => assertSourceCheckout('a'.repeat(40), dir, { PATH: dir }), error =>
    error.message === 'Source checkout check failed');
  assert.throws(() => assertSourceCheckout('a'.repeat(40), dir), /Source checkout check failed: git/);
});

test('real extraction child gets explicit environment and privileged diagnostics stay bounded', async t => {
  const { fixture } = await import('./helpers/train-fixture.mjs');
  const { restoreTrainArtifacts } = await import('../../tools/ci/pipeline/train-restore.mjs');
  const { createHash } = await import('node:crypto');
  const { mkdir } = await import('node:fs/promises');
  const f = await fixture(t), run = { ...f.observation.run, run_attempt: 1,
    repository: { id: 11 }, head_repository: { id: 11 } };
  const bin = path.join(f.directory, 'proxy'); await mkdir(bin);
  const log = path.join(bin, 'observed');
  await writeFile(path.join(bin, 'python3'), `#!${process.execPath}\nrequire('node:fs').writeFileSync(${JSON.stringify(log)},JSON.stringify({safe:process.env.FIXTURE_SAFE==='yes',owner:Object.hasOwn(process.env,'FIXTURE_OWNER')}));console.error('fixture-sensitive-diagnostic');process.exit(1);\n`, { mode: 0o700 });
  const acquisition = path.join(f.directory, 'acquisition'); await mkdir(acquisition);
  const bytes = Buffer.from('fixture');
  const item = { id: 456, name: 'product-hosted-linux', archive: 'product-hosted-linux.zip',
    bytes: bytes.length, digest: `sha256:${createHash('sha256').update(bytes).digest('hex')}` };
  await writeFile(path.join(acquisition, item.archive), bytes);
  await writeFile(path.join(acquisition, 'acquisition.json'), JSON.stringify({
    schema: 'conduit.train-artifact-acquisition/v1', repository: f.manifest.repository, repositoryId: 11,
    manifestDigest: f.manifest.digest, source: f.source, tree: f.manifest.tree,
    runId: run.id, runAttempt: 1, artifacts: [item] }));
  const metadata = async endpoint => endpoint.includes('/artifacts/') ? {
    id: item.id, name: item.name, expired: false, size_in_bytes: item.bytes, digest: item.digest,
    workflow_run: { id: run.id, head_sha: f.source, repository_id: 11, head_repository_id: 11 } } : run;
  await assert.rejects(restoreTrainArtifacts({ manifest: f.manifest, observation: { run }, acquisition,
    directory: path.join(f.directory, 'restore'), targets: [{ id: 'hosted-linux' }],
    validate: value => value, metadata, sourceEnvironment: { PATH: bin, FIXTURE_SAFE: 'yes' } }),
    error => error.message === 'Safe artifact restoration refused');
  const seen = JSON.parse(await readFile(log, 'utf8'));
  assert.ok(seen.safe && !seen.owner);
});
