import assert from 'node:assert/strict';
import { createWriteStream, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { TARGETS } from '../targets.mjs';

export const LANES = Object.freeze([
  { id: 'preflight', runner: 'ubuntu-24.04' },
  { id: 'unit', runner: 'ubuntu-24.04' },
  ...TARGETS.map(({ id, runner }) => ({ id, runner })),
]);
export function matrix(selection) {
  const requested = selection === 'all' ? LANES.map(lane => lane.id) : selection.split(',');
  assert(requested.length && new Set(requested).size === requested.length, 'duplicate/empty selection');
  return { include: requested.map(id => {
    const lane = LANES.find(item => item.id === id);
    assert(lane, `unknown acquisition target: ${id}`);
    return lane;
  }) };
}
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  return value;
}
export function compare(cold, warm) {
  for (const report of [cold, warm]) {
    assert.equal(report.schema, 'conduit.acquisition-benchmark@1');
    assert.equal(report.outcome, 'success', 'setup must really succeed');
    assert(/^[a-f0-9]{40}$/.test(report.sourceSha), 'full source SHA required');
    assert(report.cacheKey && report.runnerImage?.version, 'cache and runner image identity required');
    for (const name of ['restoreMs', 'setupMs', 'saveMs']) assert(Number.isFinite(report[name]) && report[name] >= 0, `invalid ${name}`);
    assert.equal(report.acquisition?.schema, 'conduit.tool-acquisition/v1');
    assert.equal(report.acquisition.outcome, 'success', 'acquisition receipt must succeed');
    assert.equal(report.acquisition.target, report.target, 'receipt target mismatch');
    assert.equal(report.acquisition.sourceSha, report.sourceSha, 'receipt source mismatch');
    assert.deepEqual(report.acquisition.runnerImage, report.runnerImage, 'receipt runner mismatch');
    assert(report.acquisition && report.acquisition.tools && Object.keys(report.acquisition.tools).length, 'installed tool identities required');
  }
  assert.equal(cold.phase, 'cold');
  assert.equal(warm.phase, 'warm');
  assert.equal(cold.cacheHit, false, 'cold must not restore project cache');
  assert.equal(warm.cacheHit, true, 'warm requires exact cache hit');
  for (const name of ['target', 'sourceSha', 'cacheKey', 'runner']) assert.equal(cold[name], warm[name], `${name} changed`);
  assert.deepEqual(cold.runnerImage, warm.runnerImage, 'runner image changed: measurements not comparable');
  assert.deepEqual(canonical([...cold.acquisition.tools].sort((a, b) => a.name.localeCompare(b.name))), canonical([...warm.acquisition.tools].sort((a, b) => a.name.localeCompare(b.name))), 'installed tool identity changed');
  assert.equal(cold.acquisition.specificationKey, warm.acquisition.specificationKey, 'tool specification changed');
  assert(cold.acquisition.specificationKey, 'tool specification identity required');
  const counts = report => {
    const result = {};
    for (const op of report.acquisition.operations ?? []) {
      const key = `${op.kind}/${op.outcome}`;
      result[key] = (result[key] ?? 0) + 1;
    }
    return result;
  };
  const total = report => report.restoreMs + report.setupMs + report.saveMs;
  return {
    schema: 'conduit.acquisition-comparison@1', target: cold.target, sourceSha: cold.sourceSha,
    cacheKey: cold.cacheKey, runnerImage: cold.runnerImage,
    coldPreparationMs: total(cold), warmPreparationMs: total(warm),
    savedMs: total(cold) - total(warm),
    operationCounts: { cold: counts(cold), warm: counts(warm) },
    speedup: total(warm) > 0 ? total(cold) / total(warm) : null,
    interpretation: 'Project-cache cold versus exact restored-cache warm on fresh hosted runners; runner image tools are preinstalled. Acquisition only, not product build or proof. Checkout and baseline Rust/Node provisioning are excluded from these preparation durations.',
    cold, warm,
  };
}
const read = file => JSON.parse(readFileSync(file, 'utf8'));
const write = (file, data) => writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
function state(directory) { return path.join(directory, 'benchmark.json'); }
async function runSetup(target, directory) {
  const args = ['xtask', 'ci', 'pipeline', ...(target === 'preflight' ? ['setup-ci'] : target === 'unit' ? ['setup-unit'] : ['setup', target])];
  const log = createWriteStream(path.join(directory, 'setup.log'));
  const started = performance.now();
  const code = await new Promise((resolve, reject) => {
    const child = spawn('cargo', args, { stdio: ['ignore', 'pipe', 'pipe'] });
    for (const stream of [child.stdout, child.stderr]) stream.on('data', chunk => { process.stdout.write(chunk); log.write(chunk); });
    child.on('error', reject);
    child.on('close', resolve);
  }).finally(() => log.end());
  const report = read(state(directory));
  report.setupMs = performance.now() - started;
  report.outcome = code === 0 ? 'success' : 'failed';
  const acquisition = `target/acquisition/${target}.json`;
  if (existsSync(acquisition)) report.acquisition = read(acquisition);
  write(state(directory), report);
  assert.equal(code, 0, 'real acquisition failed; retained setup.log');
}
export async function main([command, ...args]) {
  if (command === 'matrix') { console.log(JSON.stringify(matrix(args[0]))); return; }
  if (command === 'compare') { write(args[2], compare(read(args[0]), read(args[1]))); return; }
  const [directory] = args;
  if (command === 'begin') {
    mkdirSync(directory, { recursive: true });
    write(state(directory), {
      schema: 'conduit.acquisition-benchmark@1', phase: process.env.BENCHMARK_PHASE,
      target: process.env.TARGET, sourceSha: process.env.SOURCE_SHA || process.env.GITHUB_SHA,
      runner: process.env.BENCHMARK_RUNNER,
      runnerImage: { os: process.env.ImageOS, version: process.env.ImageVersion },
      cacheKey: process.env.BENCHMARK_KEY, cacheHit: false,
      restoreMs: 0, setupMs: 0, saveMs: 0, outcome: 'incomplete', timestamp: Date.now(),
    });
    return;
  }
  if (command === 'setup') { await runSetup(args[1], directory); return; }
  const report = read(state(directory));
  if (command === 'restored') { report.restoreMs = Date.now() - report.timestamp; report.cacheHit = process.env.CACHE_HIT === 'true'; }
  else if (command === 'saving') report.timestamp = Date.now();
  else if (command === 'saved') report.saveMs = Date.now() - report.timestamp;
  else throw new Error('unknown benchmark command');
  write(state(directory), report);
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main(process.argv.slice(2)).catch(error => { console.error(error.message); process.exitCode = 1; });
}
