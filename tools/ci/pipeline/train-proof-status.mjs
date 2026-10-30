// Workflow-dispatch job checks are not eligible branch gates. The exhaustive
// aggregate alone reports this independent Actions-authored commit status.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const fail = () => { throw new Error('Exact Integration proof status required'); };
const positive = n => Number.isSafeInteger(n) && n > 0;
export function proofStatusIdentity(repository, source, runId, attempt) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? '') || !/^[a-f0-9]{40}$/.test(source ?? '')
      || !positive(runId) || !positive(attempt)) fail();
  return { state: 'success', context: 'train-proof',
    description: `Integration ${runId} attempt ${attempt}`,
    target_url: `https://github.com/${repository}/actions/runs/${runId}/attempts/${attempt}` };
}
export function verifyTrainProofStatus(statuses, run, repository) {
  const expected = proofStatusIdentity(repository, run.head_sha, run.id, run.run_attempt);
  if (!Array.isArray(statuses) || statuses.length >= 100) fail();
  const status = statuses.find(value => value.context === expected.context);
  if (!status || !positive(status.id) || status.url !== `https://api.github.com/repos/${repository}/statuses/${run.head_sha}`
      || status.creator?.id !== 41898282 || status.creator?.login !== 'github-actions[bot]'
      || status.creator?.type !== 'Bot'
      || Object.entries(expected).some(([key, value]) => status[key] !== value)) fail();
  return { statusId: status.id, ...expected };
}
export function reportTrainProofStatus(env = process.env, command = spawnSync) {
  if (!['push', 'workflow_dispatch'].includes(env.GITHUB_EVENT_NAME)
      || env.GITHUB_JOB !== 'train-accepted') fail();
  const body = proofStatusIdentity(env.GITHUB_REPOSITORY, env.GITHUB_SHA,
    Number(env.GITHUB_RUN_ID), Number(env.GITHUB_RUN_ATTEMPT));
  const result = command('gh', ['api', `repos/${env.GITHUB_REPOSITORY}/statuses/${env.GITHUB_SHA}`,
    '--method', 'POST', '--input', '-'], { encoding: 'utf8', timeout: 60000,
    maxBuffer: 2 * 1024 ** 2, input: JSON.stringify(body) });
  if (result.error || result.signal || result.status !== 0) fail();
  verifyTrainProofStatus([JSON.parse(result.stdout)], { head_sha: env.GITHUB_SHA,
    id: Number(env.GITHUB_RUN_ID), run_attempt: Number(env.GITHUB_RUN_ATTEMPT) }, env.GITHUB_REPOSITORY);
}
if (process.argv[1] === fileURLToPath(import.meta.url)) reportTrainProofStatus();
