// GitHub copies successful sibling jobs into a failed-jobs rerun. The copied
// job has a new ID and attempt, but its original execution times are retained.
export async function retainedProductRun({ run, producer, target, artifact, metadata, manifestDigest }) {
  const attempt = producer?.run?.attempt;
  if (attempt === run.run_attempt) return run;
  const fail = () => { throw new Error('Product producer run identity mismatch'); };
  if (manifestDigest !== null || run.event !== 'push' || run.head_branch !== 'dev'
      || !Number.isSafeInteger(attempt) || attempt < 1 || attempt >= run.run_attempt
      || producer.run.job !== 'target') fail();

  const name = `proof / target / ${target}`;
  const endpoint = `repos/${run.repository.full_name}/actions/runs/${run.id}/attempts`;
  async function upload(attemptNumber) {
    const inventory = await metadata(`${endpoint}/${attemptNumber}/jobs?per_page=100`);
    if (!Array.isArray(inventory.jobs) || inventory.jobs.length > 100
        || inventory.total_count !== inventory.jobs.length) fail();
    const matching = inventory.jobs.filter(job => job.name === name);
    const [job] = matching;
    if (matching.length !== 1 || job.run_id !== run.id || job.run_attempt !== attemptNumber
        || job.head_sha !== run.head_sha || job.status !== 'completed'
        || job.conclusion !== 'success' || !Array.isArray(job.steps)) fail();
    const uploads = job.steps.filter(step => /^Run actions\/upload-artifact@/.test(step.name)
        && step.status === 'completed' && step.conclusion === 'success');
    const [step] = uploads;
    if (uploads.length !== 1) fail();
    const times = [job.started_at, step.started_at, artifact.created_at, step.completed_at, job.completed_at].map(Date.parse);
    if (times.some((time, index) => !Number.isFinite(time) || (index && time < times[index - 1]))) fail();
    return times;
  }

  const original = await upload(attempt);
  const retained = await upload(run.run_attempt);
  if (!original.every((time, index) => time === retained[index])) fail();
  return { ...run, run_attempt: attempt };
}
