// This tier admits inputs to full combined proof. It never satisfies a branch
// check or substitutes for exhaustive train products and landing protection.
import { TARGETS } from './targets.mjs';
import { UNIT_SHARDS } from './plan.mjs';
const fail = reason => { throw new Error(`Candidate admission: ${reason}`); };
const positive = value => Number.isSafeInteger(value) && value > 0;
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const keys = (value, expected) => value && equal(Object.keys(value).sort(), [...expected].sort());

function classify(jobs, conclusion) {
  if (!Array.isArray(jobs) || jobs.length < 6 || jobs.length > 100) fail('incomplete or unbounded job inventory');
  const ids = new Set(), names = new Map(), targets = new Map(), units = new Set();
  for (const job of jobs) {
    if (!keys(job, ['id', 'name', 'conclusion']) || !positive(job.id) || typeof job.name !== 'string' ||
        !['success', 'failure', 'skipped'].includes(job.conclusion) || ids.has(job.id) || names.has(job.name)) fail('ambiguous or invalid job identity');
    ids.add(job.id); names.set(job.name, job.conclusion);
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
  if (names.get('proof / candidate/quick') !== 'success') fail('preflight must pass');
  if (units.size) {
    if (names.has('proof / unit') || units.size !== UNIT_SHARDS.length ||
        [...units].some(unit => names.get(`proof / unit (${unit})`) !== 'success')) fail('selected portable unit matrix must completely pass');
  } else if (names.get('proof / unit') !== 'skipped') fail('missing portable unit selection or skipped placeholder');
  const placeholders = ['proof / target', 'proof / target / ${{ matrix.id }}'].filter(name => names.has(name));
  if (targets.size) {
    if (placeholders.length) fail('ambiguous target selection');
    const families = new Set([...targets.keys()].map(id => TARGETS.find(item => item.id === id).family));
    if (TARGETS.some(item => families.has(item.family) && !targets.has(item.id))) fail('incomplete selected product family');
  } else if (placeholders.length !== 1 || names.get(placeholders[0]) !== 'skipped') fail('missing target selection or skipped placeholder');
  const deferred = [];
  for (const [id, job] of targets) {
    if (job.conclusion === 'success') continue;
    if (conclusion === 'failure' && job.conclusion === 'failure' && TARGETS.find(item => item.id === id).family === 'conduitos') deferred.push(id);
    else fail('selected host or non-native product check did not pass');
  }
  // All other selected checks must pass. Skipping the admission package corpus
  // is allowed only for an entirely unselected successful documentation run.
  if (names.get('proof / admission') !== 'success' &&
      !(conclusion === 'success' && names.get('proof / admission') === 'skipped' && !targets.size && !units.size)) fail('admission must pass');
  const wrappers = ['proof / complete', 'candidate'];
  if (conclusion === 'success') {
    if (deferred.length || wrappers.some(name => names.get(name) !== 'success')) fail('successful Candidate wrappers differ');
  } else if (conclusion === 'failure') {
    if (!deferred.length || wrappers.some(name => names.get(name) !== 'failure')) fail('failure is not confined to native products and aggregate wrappers');
  } else fail('Candidate must be completed success or bounded native failure');
  return deferred.sort();
}

export function canonicalCandidateAdmission(value) {
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
  const deferred = classify(jobs, run.conclusion);
  return canonicalCandidateAdmission({ runId: run.id, runAttempt: run.run_attempt, head: run.head_sha,
    conclusion: run.conclusion, tier: deferred.length ? 'native-deferred' : 'complete-candidate', jobs,
    deferredNativeProducts: deferred });
}
