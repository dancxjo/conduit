import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fixture } from './train-fixture.mjs';
import { createTrainCommands } from '../../../tools/ci/pipeline/train-command.mjs';

export async function commands(t) {
  const f = await fixture(t);
  const output = await mkdtemp(path.join(os.tmpdir(), 'conduit-train-output-'));
  t.after(() => rm(output, { recursive: true, force: true }));
  const calls = [];
  const runs = f.selections.map((selection, i) => [{ ...selection.candidateRun, id: i + 10 }]);
  const command = (program, args, options) => {
    if (program === 'git') return spawnSync(program, args, options);
    assert.equal(program, 'gh'); assert.equal(args[0], 'api'); assert.equal(args.length, 2);
    calls.push(args[1]);
    let value;
    if (args[1].endsWith('/git/ref/heads/dev')) value = { object: { sha: f.base } };
    else if (args[1].endsWith('/actions/runs/123')) value = f.observation.run;
    else if (args[1].includes('/pulls/')) value = f.selections.find(s => args[1].endsWith(`/pulls/${s.pr.number}`))?.pr;
    else if (args[1].includes('/actions/workflows/candidate.yml/runs?')) {
      const index = f.selections.findIndex(s => args[1].includes(`head_sha=${s.pr.head.sha}`));
      value = { total_count: runs[index].length, workflow_runs: runs[index] };
    } else if (/\/actions\/runs\/\d+\/attempts\/\d+\/jobs\?per_page=100$/.test(args[1])) {
      const selected = f.selections.find(s => args[1].includes(`/runs/${s.candidateRun.id}/`));
      const run = runs[f.selections.indexOf(selected)].at(-1);
      value = { ...selected.candidateJobs, jobs: selected.candidateJobs.jobs.map(job => ({ ...job,
        run_id: run.id, run_attempt: run.run_attempt, head_sha: run.head_sha })) };
    } else if (/\/actions\/runs\/\d+$/.test(args[1])) {
      value = runs.flat().find(run => args[1].endsWith(`/runs/${run.id}`));
    } else assert.fail(`Unexpected API ${args[1]}`);
    return { status: 0, stdout: JSON.stringify(value) };
  };
  const api = createTrainCommands({ command, directory: f.directory });
  return { ...f, output, calls, runs, api, command, filename: path.join(output, 'train.json') };
}
