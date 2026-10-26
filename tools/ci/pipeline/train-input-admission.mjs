// This tier admits inputs to full combined proof. It never satisfies a branch
// check or substitutes for exhaustive train products and landing protection.
import { TARGETS } from './targets.mjs';
import { UNIT_SHARDS } from './plan.mjs';
export class CandidateNotReadyError extends Error {}
const fail = reason => { throw new Error(`Candidate admission: ${reason}`); };
const notReady = (reason, conclusion, failed) => {
  if (conclusion === 'failure' && failed) throw new CandidateNotReadyError(`Candidate admission: ${reason}`);
  fail(reason);
};
const positive = value => Number.isSafeInteger(value) && value > 0;
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
export const CHEAP_INPUT_SCHEMA = 'conduit.candidate-cheap-input/v1';
export const CHEAP_INPUT_JOB = 'cheap-input/v1';
const keys = (value, expected) => value && equal(Object.keys(value).sort(), [...expected].sort());

function classify(jobs, conclusion) {
  if (!Array.isArray(jobs) || jobs.length < 6 || jobs.length > 100) fail('incomplete or unbounded job inventory');
  const ids = new Set(), names = new Map(), targets = new Map(), units = new Set();
  for (const job of jobs) {
    if (!keys(job, ['id', 'name', 'conclusion']) || !positive(job.id) || typeof job.name !== 'string' ||
        !['success', 'failure', 'skipped'].includes(job.conclusion) || ids.has(job.id) || names.has(job.name)) fail('ambiguous or invalid job identity');
    ids.add(job.id); names.set(job.name, job.conclusion);
    if (job.name === CHEAP_INPUT_JOB && job.conclusion === 'skipped') continue;
    if (['proof / candidate/quick', 'proof / admission', 'proof / complete', 'candidate', 'proof / unit', 'proof / target', 'proof / target / ${{ matrix.id }}'].includes(job.name)) continue;
    const unit = /^proof \/ unit \(([^)]+)\)$/.exec(job.name);
    if (unit && UNIT_SHARDS.includes(unit[1])) { units.add(unit[1]); continue; }
    const target = TARGETS.find(item => job.name === `proof / target / ${item.id}`);
    if (target) { targets.set(target.id, job); continue; }
    fail(`unknown job ${job.name}`);
  }
  for (const name of ['proof / candidate/quick', 'proof / admission', 'proof / complete', 'candidate']) {
    if (!names.has(name)) fail(`missing required job ${name}`);
  }
  // Establish every selected identity and complete matrix before classifying
  // an actual failed check as merely unready. A red check must never hide
  // truncated or ambiguous observations elsewhere in this Candidate.
  if (units.size) {
    if (names.has('proof / unit') || units.size !== UNIT_SHARDS.length) fail('selected portable unit matrix must completely pass');
  } else if (names.get('proof / unit') !== 'skipped') fail('missing portable unit selection or skipped placeholder');
  const placeholders = ['proof / target', 'proof / target / ${{ matrix.id }}'].filter(name => names.has(name));
  if (targets.size) {
    if (placeholders.length) fail('ambiguous target selection');
    const families = new Set([...targets.keys()].map(id => TARGETS.find(item => item.id === id).family));
    if (TARGETS.some(item => families.has(item.family) && !targets.has(item.id))) fail('incomplete selected product family');
  } else if (placeholders.length !== 1 || names.get(placeholders[0]) !== 'skipped') fail('missing target selection or skipped placeholder');
  const wrappers = ['proof / complete', 'candidate'];
  if (conclusion === 'success') {
    if (wrappers.some(name => names.get(name) !== 'success')) fail('successful Candidate wrappers differ');
  } else if (conclusion === 'failure') {
    if (wrappers.some(name => names.get(name) !== 'failure')) fail('failure is not confined to native products and aggregate wrappers');
  } else fail('Candidate must be completed success or bounded native failure');
  if (names.get('proof / candidate/quick') !== 'success') notReady('preflight must pass', conclusion, names.get('proof / candidate/quick') === 'failure');
  if (units.size && [...units].some(unit => names.get(`proof / unit (${unit})`) !== 'success')) {
    notReady('selected portable unit matrix must completely pass', conclusion,
      [...units].some(unit => names.get(`proof / unit (${unit})`) === 'failure'));
  }
  const deferred = [];
  for (const [id, job] of targets) {
    if (job.conclusion === 'success') continue;
    if (conclusion === 'failure' && job.conclusion === 'failure' && TARGETS.find(item => item.id === id).family === 'conduitos') deferred.push(id);
    else notReady('selected host or non-native product check did not pass', conclusion, job.conclusion === 'failure');
  }
  // Skipped admission is allowed only for wholly unselected successful docs.
  if (names.get('proof / admission') !== 'success' &&
      !(conclusion === 'success' && names.get('proof / admission') === 'skipped' && !targets.size && !units.size)) {
    notReady('admission must pass', conclusion, names.get('proof / admission') === 'failure');
  }
  if (conclusion === 'failure' && !deferred.length) fail('failure is not confined to native products and aggregate wrappers');
  return deferred.sort();
}

function cheapJobs(jobs, conclusion) {
  if (!Array.isArray(jobs) || jobs.length !== 3) fail('cheap input requires successful complete job inventory');
  const expected = new Map([[CHEAP_INPUT_JOB, 'success'], ['candidate', 'success'], ['proof', 'skipped']]);
  const ids = new Set(), results = new Map();
  for (const job of jobs) {
    if (!keys(job, ['id', 'name', 'conclusion']) || !positive(job.id) || ids.has(job.id)
        || !expected.has(job.name) || results.has(job.name)
        || !['success', 'failure', 'skipped'].includes(job.conclusion)) fail('cheap input job identity or result differs');
    ids.add(job.id); results.set(job.name, job.conclusion);
  }
  if (results.size !== expected.size) fail('missing cheap input job');
  if (results.get('proof') !== 'skipped') fail('cheap input job identity or result differs');
  if (conclusion !== 'success') {
    if (conclusion === 'failure' && results.get('candidate') === 'failure' && results.get(CHEAP_INPUT_JOB) === 'failure') {
      throw new CandidateNotReadyError('Candidate admission: cheap input requires successful complete job inventory');
    }
    fail('cheap input requires successful complete job inventory');
  }
  if ([...expected].some(([name, value]) => results.get(name) !== value)) fail('cheap input job identity or result differs');
}

export function canonicalCandidateAdmission(value) {
  if (value?.schema === CHEAP_INPUT_SCHEMA) {
    if (!keys(value, ['schema', 'runId', 'runAttempt', 'head', 'conclusion', 'tier', 'jobs', 'deferredNativeProducts'])
        || !positive(value.runId) || !positive(value.runAttempt) || !/^[a-f0-9]{40}$/.test(value.head ?? '')
        || value.tier !== 'cheap-input-v1' || !equal(value.deferredNativeProducts, [])) fail('invalid versioned cheap input custody');
    cheapJobs(value.jobs, value.conclusion);
    return { schema: CHEAP_INPUT_SCHEMA, runId: value.runId, runAttempt: value.runAttempt, head: value.head,
      conclusion: value.conclusion, tier: value.tier, jobs: value.jobs.map(job => ({ ...job })).sort((a, b) => a.id - b.id), deferredNativeProducts: [] };
  }
  if (!keys(value, ['runId', 'runAttempt', 'head', 'conclusion', 'tier', 'jobs', 'deferredNativeProducts']) ||
      !positive(value.runId) || !positive(value.runAttempt) || !/^[a-f0-9]{40}$/.test(value.head ?? '')) fail('missing exact run/attempt/head custody');
  const deferred = classify(value.jobs, value.conclusion);
  const tier = deferred.length ? 'native-deferred' : 'complete-candidate';
  if (value.tier !== tier || !equal(value.deferredNativeProducts, deferred)) fail('admission tier differs from actual jobs');
  return { runId: value.runId, runAttempt: value.runAttempt, head: value.head, conclusion: value.conclusion,
    tier, jobs: value.jobs.map(job => ({ id: job.id, name: job.name, conclusion: job.conclusion })).sort((a, b) => a.id - b.id),
    deferredNativeProducts: deferred };
}

export function candidateAdmission(run, inventory) {
  if (!positive(run?.id) || !positive(run.run_attempt) || run.status !== 'completed' ||
      !Array.isArray(inventory?.jobs) || inventory.total_count !== inventory.jobs.length ||
      inventory.jobs.length > 100) fail('incomplete exact-attempt job inventory');
  const jobs = inventory.jobs.map(job => {
    if (job.run_id !== run.id || job.run_attempt !== run.run_attempt || job.head_sha !== run.head_sha || job.status !== 'completed') fail('job run, attempt, head or terminal state differs');
    return { id: job.id, name: job.name, conclusion: job.conclusion };
  });
  if (jobs.some(job => job.name === CHEAP_INPUT_JOB && job.conclusion !== 'skipped')) {
    return canonicalCandidateAdmission({ schema: CHEAP_INPUT_SCHEMA, runId: run.id, runAttempt: run.run_attempt,
      head: run.head_sha, conclusion: run.conclusion, tier: 'cheap-input-v1', jobs, deferredNativeProducts: [] });
  }
  const deferred = classify(jobs, run.conclusion);
  return canonicalCandidateAdmission({ runId: run.id, runAttempt: run.run_attempt, head: run.head_sha,
    conclusion: run.conclusion, tier: deferred.length ? 'native-deferred' : 'complete-candidate', jobs,
    deferredNativeProducts: deferred });
}
