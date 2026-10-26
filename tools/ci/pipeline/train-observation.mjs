// Read-only, bounded live Candidate observations shared by controller and proof
// scheduling. This module accepts no writer/policy credential or mutation API.
const fail = message => { throw new Error(message); };
const number = value => {
  if (!/^[1-9][0-9]*$/.test(String(value)) || !Number.isSafeInteger(Number(value))) fail('Invalid numeric identity');
  return Number(value);
};
export function readTrainSelections(repository, numbers, api) {
  if (!Array.isArray(numbers) || numbers.length < 1 || numbers.length > 32) fail('Train requires 1..32 pull requests');
  const normalized = numbers.map(number);
  if (new Set(normalized).size !== normalized.length) fail('Duplicate pull request');
  if (typeof repository !== 'string' || repository.length > 512 || !/^[\w.-]+\/[\w.-]+$/.test(repository)) fail('Invalid repository');
  const base = `repos/${repository}`;
  return normalized.map(id => {
    const pr = api(`${base}/pulls/${id}`);
    if (!/^[a-f0-9]{40}$/.test(pr.head?.sha ?? '')) fail('Invalid live PR head');
    const response = api(`${base}/actions/workflows/candidate.yml/runs?head_sha=${pr.head.sha}&event=pull_request&per_page=100`);
    if (!Array.isArray(response.workflow_runs) || response.workflow_runs.length > 100 ||
        !Number.isSafeInteger(response.total_count) || response.total_count !== response.workflow_runs.length) fail('Invalid Candidate run inventory');
    // Never select an older green run over a newer failed/running attempt.
    const matching = response.workflow_runs.filter(run => run.head_sha === pr.head.sha);
    for (const run of matching) number(run.id);
    const latest = matching.sort((a, b) => b.id - a.id)[0];
    if (!latest) fail('No exact-head Candidate run');
    const run = api(`${base}/actions/runs/${number(latest.id)}`);
    if (run.id !== latest.id || run.head_sha !== latest.head_sha || run.run_attempt !== latest.run_attempt ||
        run.status !== latest.status || run.conclusion !== latest.conclusion) fail('Candidate run or attempt changed');
    const jobs = api(`${base}/actions/runs/${number(run.id)}/attempts/${number(run.run_attempt)}/jobs?per_page=100`);
    if (!Array.isArray(jobs.jobs) || jobs.jobs.length > 100 || !Number.isSafeInteger(jobs.total_count) ||
        jobs.total_count !== jobs.jobs.length) fail('Incomplete Candidate attempt job inventory');
    const after = api(`${base}/actions/runs/${number(run.id)}`);
    if (after.id !== run.id || after.head_sha !== run.head_sha || after.run_attempt !== run.run_attempt ||
        after.status !== run.status || after.conclusion !== run.conclusion) fail('Candidate run or attempt changed while observing jobs');
    const finalInventory = api(`${base}/actions/workflows/candidate.yml/runs?head_sha=${pr.head.sha}&event=pull_request&per_page=100`);
    if (!Array.isArray(finalInventory.workflow_runs) || finalInventory.workflow_runs.length > 100 ||
        !Number.isSafeInteger(finalInventory.total_count) || finalInventory.total_count !== finalInventory.workflow_runs.length) fail('Incomplete final Candidate run inventory');
    const finalLatest = finalInventory.workflow_runs.filter(value => value.head_sha === pr.head.sha)
      .sort((a, b) => number(b.id) - number(a.id))[0];
    if (!finalLatest || finalLatest.id !== after.id || finalLatest.run_attempt !== after.run_attempt ||
        finalLatest.status !== after.status || finalLatest.conclusion !== after.conclusion) fail('Latest Candidate changed while observing jobs');
    return { pr, candidateRun: after, candidateJobs: jobs };
  });
}
