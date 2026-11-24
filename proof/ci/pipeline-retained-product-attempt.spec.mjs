import assert from 'node:assert/strict';
import test from 'node:test';
import { retainedProductRun } from '../../tools/ci/pipeline/retained-product-attempt.mjs';

function fixture() {
  const run = { id: 42, run_attempt: 2, event: 'push', head_branch: 'dev',
    head_sha: 'a'.repeat(40), repository: { full_name: 'owner/repo' } };
  const job = { name: 'proof / target / hosted-linux', head_sha: run.head_sha, run_id: run.id,
    status: 'completed', conclusion: 'success', started_at: '2026-11-24T00:00:00Z',
    completed_at: '2026-11-24T00:01:00Z', steps: [{ name: 'Run actions/upload-artifact@v7',
      status: 'completed', conclusion: 'success', started_at: '2026-11-24T00:00:54Z', completed_at: '2026-11-24T00:00:59Z' }] };
  const earlier = { ...job, id: 9, run_attempt: 1 };
  const latest = { ...job, id: 10, run_attempt: 2 };
  return { earlier, latest, inputs: { run, producer: { run: { attempt: 1, job: 'target' } },
    target: 'hosted-linux', artifact: { created_at: '2026-11-24T00:00:57Z' }, manifestDigest: null,
    metadata: async endpoint => ({ total_count: 1, jobs: [endpoint.includes('/attempts/1/') ? earlier : latest] }) } };
}

test('ordinary publication accepts a successful inherited product on the same source', async () => {
  const { inputs } = fixture();
  assert.deepEqual(await retainedProductRun(inputs), { ...inputs.run, run_attempt: 1 });
});

for (const [description, change] of [
  ['target reran', f => { f.latest.completed_at = '2026-11-24T00:02:00Z'; }],
  ['earlier job failed', f => { f.earlier.conclusion = 'failure'; }],
  ['foreign source', f => { f.earlier.head_sha = 'b'.repeat(40); }],
  ['artifact uploaded outside the observed step', f => { f.inputs.artifact.created_at = '2026-11-24T00:00:53Z'; }],
  ['adopted train retains its stricter contract', f => { f.inputs.manifestDigest = 'c'.repeat(64); }],
]) test(`retained product refuses when ${description}`, async () => {
  const f = fixture(); change(f);
  await assert.rejects(retainedProductRun(f.inputs), /Product producer run identity mismatch/);
});
