import assert from 'node:assert/strict';
import test from 'node:test';
import { compare, matrix } from '../../tools/ci/pipeline/acquisition/compare.mjs';
import { TARGETS } from '../../tools/ci/pipeline/targets.mjs';

function sample(phase) {
  return {
    schema: 'conduit.acquisition-benchmark@1', phase, target: 'rp2040',
    sourceSha: 'a'.repeat(40), runner: 'ubuntu-24.04',
    runnerImage: { os: 'ubuntu24', version: '20261001.1' },
    cacheKey: 'acquisition-benchmark-1234-1-rp2040-tools-v1',
    cacheHit: phase === 'warm', outcome: 'success',
    restoreMs: phase === 'cold' ? 100 : 900,
    setupMs: phase === 'cold' ? 10000 : 2000,
    saveMs: phase === 'cold' ? 500 : 0,
    acquisition: {
      schema: 'conduit.tool-acquisition/v1', target: 'rp2040', sourceSha: 'a'.repeat(40),
      runnerImage: { os: 'ubuntu24', version: '20261001.1' }, outcome: 'success',
      specificationKey: 'tools-v1', tools: [{ name: 'elf2uf2-rs', version: '2.2.0', sha256: 'b'.repeat(64) }],
      operations: [{ kind: 'install', outcome: 'success', durationMs: 1000 }],
    },
  };
}
test('all covers each canonical runner/target plus unit and preflight; inputs cannot inject matrix entries', () => {
  const all = matrix('all').include;
  assert.deepEqual(all.slice(2), TARGETS.map(({ id, runner }) => ({ id, runner })));
  assert.deepEqual(all.slice(0, 2).map(lane => lane.id), ['preflight', 'unit']);
  assert.deepEqual(matrix('hosted-windows,conduitos-loongarch64').include.map(lane => lane.runner), ['windows-2025', 'ubuntu-26.04']);
  for (const invalid of ['', 'unit,unit', 'unit,', 'unit;echo injected', '../setup']) assert.throws(() => matrix(invalid));
});
test('comparison includes cache overhead and preserves honest negative savings', () => {
  const cold = sample('cold');
  const warm = sample('warm');
  const result = compare(cold, warm);
  assert.equal(result.coldPreparationMs, 10600);
  assert.equal(result.warmPreparationMs, 2900);
  assert.equal(result.savedMs, 7700);
  assert.equal(result.operationCounts.cold['install/success'], 1);
  warm.restoreMs = 20000;
  assert(compare(cold, warm).savedMs < 0);
});
test('identity object ordering and tool ordering do not invalidate the same installed closure', () => {
  const cold = sample('cold');
  const warm = sample('warm');
  cold.acquisition.tools.push({ name: 'rustc', version: '1.98.1' });
  warm.acquisition.tools.unshift({ version: '1.98.1', name: 'rustc' });
  compare(cold, warm);
});
for (const [name, mutate] of [
  ['cold cache hit', (c) => { c.cacheHit = true; }],
  ['warm cache miss', (_, w) => { w.cacheHit = false; }],
  ['different source', (_, w) => { w.sourceSha = 'c'.repeat(40); }],
  ['runner image drift', (_, w) => { w.runnerImage.version = 'next-image'; }],
  ['tool binary drift', (_, w) => { w.acquisition.tools[0].sha256 = 'c'.repeat(64); }],
  ['specification drift', (_, w) => { w.acquisition.specificationKey = 'different'; }],
  ['failed actual setup', (_, w) => { w.outcome = 'failed'; }],
  ['failed acquisition receipt', (_, w) => { w.acquisition.outcome = 'failure'; }],
  ['missing installed identities', (_, w) => { w.acquisition.tools = []; }],
  ['invalid measured duration', (_, w) => { w.setupMs = -1; }],
]) test(`rejects ${name}`, () => {
  const cold = sample('cold');
  const warm = sample('warm');
  mutate(cold, warm);
  assert.throws(() => compare(cold, warm));
});
