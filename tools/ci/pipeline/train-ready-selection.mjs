// Explicit service policy discovers inputs; it neither assembles source nor
// authorizes landing. All observations and grants belong to the caller's
// finite authenticated read-only session.
import { readTrainSelections } from './train-observation.mjs';
import { selectTrainInputs } from './train.mjs';
import { candidateAdmission, CandidateNotReadyError } from './train-input-admission.mjs';
import { validateIntegrationRun } from './publish.mjs';

const POLICY = 'all-ready-same-repository-dev-prs';
const positive = value => Number.isSafeInteger(value) && value > 0;
const sha = value => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);
const pending = new Set(['queued', 'requested', 'waiting', 'pending', 'in_progress']);
class Refusal extends Error {}
const fail = reason => { throw new Refusal(reason); };
function runIdentity(run, repository, repositoryId, source, workflow, event) {
  if (!positive(run?.id) || !positive(run.run_attempt) || run.repository?.full_name !== repository
      || run.head_repository?.full_name !== repository || run.repository?.id !== repositoryId
      || run.head_repository?.id !== repositoryId || run.head_sha !== source
      || run.path !== `.github/workflows/${workflow}.yml` || run.event !== event
      || (!pending.has(run.status) && run.status !== 'completed')) fail('Unknown or foreign run metadata');
}
function inventory(value) {
  const runs = value?.workflow_runs;
  if (!Number.isSafeInteger(value?.total_count) || value.total_count < 0 || value.total_count > 100
      || !Array.isArray(runs) || runs.length !== value.total_count
      || runs.some(run => !positive(run?.id)) || new Set(runs.map(run => run.id)).size !== runs.length) fail('Incomplete exact-source Integration inventory');
  return runs;
}
function prClass(pr, number, repository, repositoryId) {
  if (pr?.number !== number || !['open', 'closed'].includes(pr.state) || typeof pr.draft !== 'boolean'
      || !sha(pr.head?.sha) || !sha(pr.base?.sha) || typeof pr.head?.repo?.full_name !== 'string'
      || !positive(pr.head.repo.id) || typeof pr.base?.ref !== 'string'
      || pr.base?.repo?.full_name !== repository || pr.base.repo.id !== repositoryId) fail('Unknown pull request metadata');
  if (pr.state === 'closed') return 'closed';
  if (pr.draft) return 'draft';
  if (pr.head.repo.full_name !== repository || pr.head.repo.id !== repositoryId) return 'fork';
  if (pr.base.ref !== 'dev') return 'different-base';
  if (pr.mergeable === false) return 'conflicted';
  if (pr.mergeable !== true) fail('Unknown pull request mergeability');
  return 'candidate';
}
function jobIdentities(snapshot) {
  const { candidateRun: run, candidateJobs: jobs } = snapshot;
  if (!Array.isArray(jobs?.jobs) || jobs.total_count !== jobs.jobs.length || jobs.jobs.length > 100
      || new Set(jobs.jobs.map(job => job.id)).size !== jobs.jobs.length) fail('Incomplete Candidate job inventory');
  for (const job of jobs.jobs) {
    if (!positive(job.id) || job.run_id !== run.id || job.run_attempt !== run.run_attempt
        || job.head_sha !== run.head_sha || typeof job.name !== 'string' || job.name.length > 512
        || (!pending.has(job.status) && job.status !== 'completed')) fail('Candidate job identity changed');
  }
}

export function discoverReadyTrainInputs({ policy, repository, repositoryId, api, observeCheapPolicy }) {
  const classification = [];
  try {
    if (policy !== POLICY || !/^[\w.-]+\/[\w.-]+$/.test(repository ?? '') || repository.length > 512
        || !positive(repositoryId) || typeof api !== 'function') fail('Explicit bounded ready-selection policy required');
    const read = endpoint => {
      const value = api(endpoint);
      if (value && typeof value.then === 'function') fail('Synchronous trusted observer required');
      return value;
    };
    const base = `repos/${repository}`;
    const source = read(`${base}/git/ref/heads/dev`)?.object?.sha;
    if (!sha(source)) fail('Invalid current development identity');
    const runEndpoint = `${base}/actions/workflows/integration.yml/runs?head_sha=${source}&event=push&branch=dev&per_page=100`;
    const latest = () => {
      const runs = inventory(read(runEndpoint));
      for (const run of runs) {
        runIdentity(run, repository, repositoryId, source, 'integration', 'push');
        if (run.head_branch !== 'dev') fail('Foreign development Integration branch');
      }
      return runs.sort((a, b) => b.id - a.id)[0] ?? null;
    };
    const listed = latest();
    if (!listed) return { kind: 'waiting', reason: 'current-dev-unproven', development: { source, runId: null, runAttempt: null, status: 'not-observed' }, classification };
    const accepted = read(`${base}/actions/runs/${listed.id}`);
    runIdentity(accepted, repository, repositoryId, source, 'integration', 'push');
    if (JSON.stringify([listed.id, listed.run_attempt, listed.status, listed.conclusion]) !==
        JSON.stringify([accepted.id, accepted.run_attempt, accepted.status, accepted.conclusion]) || accepted.head_branch !== 'dev') fail('Current development run changed');
    if (accepted.status !== 'completed') return { kind: 'waiting', reason: 'current-dev-integration-running',
      development: { source, runId: accepted.id, runAttempt: accepted.run_attempt, status: accepted.status }, classification };
    if (accepted.conclusion !== 'success') return { kind: 'blocked', reason: 'current-dev-integration-failed', classification };
    validateIntegrationRun(accepted, repository, source);
    const prs = read(`${base}/pulls?state=open&base=dev&per_page=100`);
    if (!Array.isArray(prs) || prs.length >= 100 || prs.some(pr => !positive(pr?.number))
        || new Set(prs.map(pr => pr.number)).size !== prs.length) fail('Incomplete open pull request inventory');
    const candidates = [];
    for (const listedPr of [...prs].sort((a, b) => a.number - b.number)) {
      const pr = read(`${base}/pulls/${listedPr.number}`);
      const state = prClass(pr, listedPr.number, repository, repositoryId);
      if (state !== 'candidate') classification.push({ number: pr.number, state });
      else candidates.push(pr);
    }
    if (candidates.length > 32) fail('Ready-selection candidate bound exceeded');
    const selected = [];
    for (const pr of candidates) {
      let snapshot;
      try { [snapshot] = readTrainSelections(repository, [pr.number], read); }
      catch (error) {
        if (error.message === 'No exact-head Candidate run') { classification.push({ number: pr.number, state: 'no-candidate' }); continue; }
        throw error;
      }
      if (prClass(snapshot.pr, pr.number, repository, repositoryId) !== 'candidate'
          || snapshot.pr.head.sha !== pr.head.sha) fail('Pull request changed during Candidate observation');
      const run = snapshot.candidateRun;
      runIdentity(run, repository, repositoryId, pr.head.sha, 'candidate', 'pull_request');
      if (!Array.isArray(run.pull_requests) || !run.pull_requests.some(value => value.number === pr.number)) fail('Candidate pull request identity differs');
      jobIdentities(snapshot);
      if (run.status !== 'completed') { classification.push({ number: pr.number, state: 'candidate-running' }); continue; }
      if (['cancelled', 'timed_out', 'action_required', 'startup_failure', 'stale'].includes(run.conclusion)) {
        classification.push({ number: pr.number, state: 'candidate-unready' }); continue;
      }
      if (!['success', 'failure'].includes(run.conclusion)) fail('Unknown Candidate conclusion');
      let admission;
      try { admission = candidateAdmission(run, snapshot.candidateJobs); }
      catch (error) {
        if (error instanceof CandidateNotReadyError) {
          classification.push({ number: pr.number, state: 'candidate-failed' }); continue;
        }
        throw error;
      }
      selected.push({ snapshot, admission });
      classification.push({ number: pr.number, state: admission.tier });
    }
    const development = { source, runId: accepted.id, runAttempt: accepted.run_attempt };
    let selection;
    if (selected.length) {
      const refreshed = readTrainSelections(repository, selected.map(value => value.snapshot.pr.number), read);
      for (const [index, snapshot] of refreshed.entries()) {
        if (prClass(snapshot.pr, selected[index].snapshot.pr.number, repository, repositoryId) !== 'candidate') fail('Ready input changed before selection');
        runIdentity(snapshot.candidateRun, repository, repositoryId, selected[index].snapshot.pr.head.sha, 'candidate', 'pull_request');
        if (JSON.stringify(candidateAdmission(snapshot.candidateRun, snapshot.candidateJobs)) !== JSON.stringify(selected[index].admission)) fail('Candidate admission changed before selection');
      }
      selection = selectTrainInputs({ repository, base: source, selections: refreshed.map(snapshot => ({ ...snapshot,
        cheapPolicy: () => {
          if (typeof observeCheapPolicy !== 'function') fail('Fresh external cheap policy observer required');
          return observeCheapPolicy({ repository });
        } })) });
    }
    if (read(`${base}/git/ref/heads/dev`)?.object?.sha !== source) fail('Development moved during ready selection');
    const final = latest();
    const current = read(`${base}/actions/runs/${accepted.id}`);
    runIdentity(current, repository, repositoryId, source, 'integration', 'push');
    validateIntegrationRun(current, repository, source);
    if (!final || final.id !== accepted.id || final.run_attempt !== accepted.run_attempt
        || final.status !== 'completed' || final.conclusion !== 'success'
        || current.run_attempt !== accepted.run_attempt || current.head_branch !== 'dev') fail('Accepted development evidence changed');
    return { kind: selection ? 'selected' : 'idle', ...(selection ? { selection } : {}), development,
      classification: classification.sort((a, b) => a.number - b.number) };
  } catch (error) {
    return { kind: 'refused', reason: error instanceof Refusal ? error.message : 'ready-selection-observation-or-policy-refused', classification };
  }
}
