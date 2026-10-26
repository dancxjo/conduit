import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, chmodSync, readFileSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const workflow = readFileSync('.github/workflows/acquisition.yml', 'utf8');
const job = name => workflow.split(`  ${name}:\n`)[1].split(/^  [\w-]+:\n/m)[0];
const step = name => workflow.split(`      - name: ${name}\n`)[1].split(/^      - (?:name|uses):/m)[0];
const blockScript = name => step(name).split('        run: |\n')[1].replace(/^          /gm, '');
const guard = step('Require accepted compiler seed before building').match(/        run: (.+)/)[1];

test('optional strict qualification routes independently while ordinary acquisition and cold product defaults remain', () => {
  const inputs = workflow.split('    inputs:\n')[1].split('\npermissions:')[0];
  for (const flag of ['validate-hosted-product', 'require-accepted-hosted-seed']) {
    assert.match(inputs.split(`      ${flag}:\n`)[1].split(/^      [\w-]+:/m)[0], /type: boolean\n        default: false/);
  }
  const routes = (name, cold, warm) => {
    const expression = job(name).match(/    if: (.+)/)[1].replace(/^\$\{\{ | \}\}$/g, '')
      .replaceAll('inputs.validate-hosted-product', String(cold))
      .replaceAll('inputs.require-accepted-hosted-seed', String(warm));
    assert.match(expression, /^[truefals!&| ()]+$/);
    return Function(`return (${expression});`)();
  };
  for (const [cold, warm, select, hosted] of [[false,false,true,false],[true,false,false,true],[false,true,false,true],[true,true,false,true]]) {
    assert.equal(routes('select', cold, warm), select);
    assert.equal(routes('hosted-product', cold, warm), hosted);
  }
  assert.match(job('measure'), /needs: \[select\]/);
  const restore = step('Restore compiler data from an accepted Integration producer');
  assert.match(restore, /if: inputs.require-accepted-hosted-seed/);
  assert.match(step('Require accepted compiler seed before building'), /if: inputs.require-accepted-hosted-seed/);
  assert.ok(workflow.indexOf('Require accepted compiler seed before building') < workflow.indexOf('Build and prove exact current-source hosted product'));
  assert.match(step('Build and prove exact current-source hosted product'), /steps\.hosted-cache-seed\.outputs\.hosted-cache-config \|\| steps\.hosted-cache\.outputs\.hosted-cache-config/);
});

test('actual pre-build shell gate refuses absent, false and malformed restore output before any build', t => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'hosted-seed-gate-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const value of ['', 'false', 'TRUE', 'unknown', 'true']) {
    const marker = path.join(root, value || 'empty');
    const result = spawnSync('bash', ['-e', '-c', `${guard}\nprintf built > "$MARKER"`], {
      env: { PATH: process.env.PATH, RESTORED: value, MARKER: marker }, encoding: 'utf8', timeout: 10000,
    });
    assert.equal(result.status === 0, value === 'true', value);
    assert.equal(existsSync(marker), value === 'true', value);
  }
});

test('token and actions permission remain scoped; accepted restore is production command without seed sealing or target caching', () => {
  assert.equal(workflow.match(/actions: read/g)?.length, 1);
  assert.doesNotMatch(workflow.split('\njobs:')[0], /actions: read|GH_TOKEN/);
  assert.match(job('hosted-product'), /permissions:\n      contents: read\n      actions: read/);
  assert.equal(workflow.match(/GH_TOKEN:/g)?.length, 1);
  const restore = step('Restore compiler data from an accepted Integration producer');
  assert.match(restore, /GH_TOKEN: \$\{\{ github.token \}\}/);
  assert.match(restore, /node tools\/ci\/pipeline\/cli.mjs compiler-cache-hosted-restore "\$CACHE_CONFIG"/);
  assert.match(restore, /conduit-hosted-acquisition-\$GITHUB_RUN_ID-\$GITHUB_RUN_ATTEMPT/);
  assert.match(restore, /conduit-hosted-restored-\$GITHUB_RUN_ID-\$GITHUB_RUN_ATTEMPT.json/);
  assert.match(restore, /durationMs/);
  assert.match(step('Build and prove exact current-source hosted product'), /node tools\/ci\/pipeline\/cli.mjs target hosted-linux "\$SOURCE_SHA"/);
  assert.doesNotMatch(job('hosted-product'), /compiler-cache-hosted-seal|Swatinem|actions\/cache|continue-on-error/);
  const retain = step('Retain current-source product and preparation evidence');
  for (const suffix of ['acquisition-\*/acquisition.json', 'transfer-\*.json']) assert.ok(retain.includes(`conduit-hosted-${suffix}`));
  assert.match(retain, /task\/selected-recipe.json/);
  assert.match(retain, /task\/cargo-cache-receipt.json/);
  assert.match(retain, /task\/hello-proof.json/);
  assert.doesNotMatch(retain, /\/objects|artifact.zip|restored\/cache|compiler-cache\//);
});

function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'hosted-reuse-qualification-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const directory = path.join(root, 'target/pipeline/hosted-linux'), acquisition = path.join(root, 'acquisition');
  mkdirSync(directory, { recursive: true }); mkdirSync(acquisition);
  const source = 'a'.repeat(40), namespace = 'b'.repeat(64), binary = 'c'.repeat(64), snapshotSha256 = 'd'.repeat(64);
  const objects = {
    acquired: { namespace, artifactId: 123, digest: `sha256:${'e'.repeat(64)}`, bytes: 456 },
    build: { namespace, snapshotSha256, before: { stats: { cache_hits: { counts: {} } } }, after: { stats: { cache_hits: { counts: { Rust: 4 } } } } },
    hello: { namespace, snapshotSha256, binarySha256: binary, binaryUnchanged: true, reportSha256: 'f'.repeat(64) },
    snapshot: { namespace, source, binarySha256: binary },
  };
  const paths = { acquired: path.join(acquisition, 'acquisition.json'), build: path.join(directory, 'cargo-cache-receipt.json'),
    hello: path.join(directory, 'hello-proof.json'), snapshot: path.join(directory, 'compiler-snapshot.json') };
  const run = () => {
    for (const [key, file] of Object.entries(paths)) writeFileSync(file, JSON.stringify(objects[key]));
    return spawnSync('bash', ['-e', '-c', blockScript('Qualify real accepted-seed compiler reuse')], {
      cwd: root, env: { PATH: process.env.PATH, SOURCE_SHA: source, ACQUISITION_DIRECTORY: acquisition },
      encoding: 'utf8', timeout: 10000,
    });
  };
  return { objects, run, receipt: path.join(directory, 'accepted-seed-qualification.json') };
}

test('actual qualification command records observed Rust hit delta and fresh product/runtime identities', t => {
  const f = fixture(t); const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  const receipt = JSON.parse(readFileSync(f.receipt, 'utf8'));
  assert.equal(receipt.rustHitDelta, 4); assert.equal(receipt.artifactId, 123);
  assert.equal(receipt.source, f.objects.snapshot.source);
  assert.equal(receipt.binarySha256, f.objects.hello.binarySha256);
  assert.match(receipt.scope, /no Integration acceptance or net speedup claimed/);
});

for (const [name, mutate] of [
  ['zero real hits', f => { f.build.after.stats.cache_hits.counts.Rust = 0; }],
  ['unchanged cumulative hits', f => { f.build.before.stats.cache_hits.counts.Rust = 4; }],
  ['missing engine statistics', f => { delete f.build.after.stats; }],
  ['invalid hit count', f => { f.build.after.stats.cache_hits.counts.Rust = -1; }],
  ['different transferred namespace', f => { f.acquired.namespace = 'wrong'; }],
  ['different current source', f => { f.snapshot.source = 'wrong'; }],
  ['different Hello snapshot', f => { f.hello.snapshotSha256 = 'wrong'; }],
  ['mutated Hello binary', f => { f.hello.binaryUnchanged = false; }],
  ['different retained binary', f => { f.hello.binarySha256 = 'wrong'; }],
]) {
  test(`actual qualification refuses ${name}`, t => {
    const f = fixture(t); mutate(f.objects);
    assert.notEqual(f.run().status, 0); assert.equal(existsSync(f.receipt), false);
  });
}

test('refused production restore preserves transfer timing and propagates failure', t => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'hosted-transfer-time-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const executable = path.join(root, 'node');
  writeFileSync(executable, '#!/bin/sh\nexit 17\n'); chmodSync(executable, 0o700);
  const result = spawnSync('bash', ['-e', '-c', blockScript('Restore compiler data from an accepted Integration producer')], {
    env: { PATH: `${root}:${process.env.PATH}`, RUNNER_TEMP: root, GITHUB_RUN_ID: '123', GITHUB_RUN_ATTEMPT: '1', CACHE_CONFIG: '/fixture/config.json' },
    encoding: 'utf8', timeout: 10000,
  });
  assert.equal(result.status, 17);
  const timing = JSON.parse(readFileSync(path.join(root, 'conduit-hosted-transfer-123-1.json'), 'utf8'));
  assert.equal(timing.scope, 'accepted-seed-discovery-download-verification-config');
  assert.ok(Number.isSafeInteger(timing.durationMs) && timing.durationMs >= 0);
});
