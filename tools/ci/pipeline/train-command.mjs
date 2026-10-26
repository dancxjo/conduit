// Read live train evidence through the repository's existing gh credentials.
// Assembly changes only a new local worktree/branch. No remote mutation occurs.
import { spawnSync } from 'node:child_process';
import { lstat, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { captureTrain, selectTrainInputs, verifyTrainManifest, verifyTrainProducts } from './train.mjs';
import { candidateAdmission } from './train-input-admission.mjs';
import { assertSourceCheckout } from './source.mjs';

const fail = message => { throw new Error(message); };
const repositoryPattern = /^[\w.-]+\/[\w.-]+$/;
const number = value => {
  if (!/^[1-9][0-9]*$/.test(String(value)) || !Number.isSafeInteger(Number(value))) fail('Invalid numeric identity');
  return Number(value);
};

export function createTrainCommands({ command = spawnSync, directory = process.cwd(), artifactCommand, observeCheapPolicy } = {}) {
  const invoke = (program, args) => {
    const result = command(program, args, { cwd: directory, encoding: 'utf8', maxBuffer: 2 * 1024 * 1024,
      ...(program === 'gh' ? { timeout: 60000 } : {}) });
    if (result.error || result.status !== 0) fail(`${program} failed: ${result.error?.message ?? result.stderr ?? result.status}`);
    return result.stdout.trim();
  };
  const api = endpoint => JSON.parse(invoke('gh', ['api', endpoint]));
  const basePath = repository => {
    if (typeof repository !== 'string' || repository.length > 512 || !repositoryPattern.test(repository)) fail('Invalid repository');
    return `repos/${repository}`;
  };
  const selection = (repository, numbers) => {
    if (!Array.isArray(numbers) || numbers.length < 1 || numbers.length > 32) fail('Train requires 1..32 pull requests');
    const normalized = numbers.map(number);
    if (new Set(normalized).size !== normalized.length) fail('Duplicate pull request');
    const base = basePath(repository);
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
      const cheap = jobs.jobs.some(job => job.name === 'cheap-input/v1' && job.conclusion !== 'skipped');
      if (cheap) candidateAdmission(after, jobs); // Refuse bad jobs before external policy observation.
      if (cheap && typeof observeCheapPolicy !== 'function') fail('Fresh external cheap policy observer required');
      return { pr, candidateRun: after, candidateJobs: jobs,
        ...(cheap ? { cheapPolicy: () => observeCheapPolicy({ repository }) } : {}) };
    });
  };
  const loadManifest = async filename => {
    const stat = await lstat(filename);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 65536) fail('Invalid train manifest file');
    const manifest = JSON.parse(await readFile(filename, 'utf8'));
    verifyTrainManifest(manifest);
    return manifest;
  };
  return {
    async restore(filename, runId, acquisition, output) {
      const manifest = await this.prepareDispatch(filename);
      const base = basePath(manifest.repository);
      const observation = {
        development: api(`${base}/git/ref/heads/dev`).object?.sha,
        selections: selection(manifest.repository, manifest.inputs.map(input => input.number)),
        run: api(`${base}/actions/runs/${number(runId)}`),
      };
      const { restoreTrainArtifacts } = await import('./train-restore.mjs');
      const restored = await restoreTrainArtifacts({ manifest, observation, acquisition,
        directory: output, metadata: api });
      await this.prepareDispatch(filename);
      return restored;
    },
    async acquire(filename, runId, output) {
      // Admission and Git reconstruction precede any download. Acquired archives
      // authenticate run-owned bytes, not complete producer build provenance.
      const manifest = await this.prepareDispatch(filename);
      const base = basePath(manifest.repository);
      const observation = {
        development: api(`${base}/git/ref/heads/dev`).object?.sha,
        selections: selection(manifest.repository, manifest.inputs.map(input => input.number)),
        run: api(`${base}/actions/runs/${number(runId)}`),
      };
      const { acquireTrainArtifacts } = await import('./train-artifacts.mjs');
      const receipt = await acquireTrainArtifacts({ manifest, observation, directory: output,
        ...(artifactCommand ? { command: artifactCommand } : {}) });
      await this.prepareDispatch(filename);
      return receipt;
    },
    async prepareDispatch(filename) {
      const manifest = await loadManifest(filename);
      assertSourceCheckout(manifest.source, directory);
      const base = basePath(manifest.repository);
      const captured = captureTrain({ repository: manifest.repository, source: manifest.source,
        base: api(`${base}/git/ref/heads/dev`).object?.sha,
        selections: selection(manifest.repository, manifest.inputs.map(input => input.number)), directory });
      if (captured.digest !== manifest.digest) fail('Train source or live selection changed before dispatch');
      return captured;
    },
    async assemble(repository, numbers, worktree, output) {
      const base = api(`${basePath(repository)}/git/ref/heads/dev`).object?.sha;
      const selected = selectTrainInputs({ repository, base, selections: selection(repository, numbers) });
      if (!path.isAbsolute(worktree) || !path.isAbsolute(output)) fail('Absolute train worktree/output paths required');
      const relative = path.relative(worktree, output);
      if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) {
        fail('Train manifest must be outside its source worktree');
      }
      // Fail before branch/worktree creation if the destination is occupied.
      for (const destination of [worktree, output]) {
        try { await lstat(destination); fail('Train destination already exists'); }
        catch (error) { if (error.code !== 'ENOENT') throw error; }
      }
      invoke('git', ['fetch', '--no-tags', 'origin', base, ...selected.inputs.map(input => input.head)]);
      if (api(`${basePath(repository)}/git/ref/heads/dev`).object?.sha !== base) fail('Development changed before assembly');
      invoke('git', ['worktree', 'add', '-b', selected.branch, '--', worktree, base]);
      try {
        for (const input of selected.inputs) {
          invoke('git', ['-C', worktree, 'merge', '--no-ff', input.head, '-m', `Train PR #${input.number}`]);
        }
        const source = invoke('git', ['-C', worktree, 'rev-parse', 'HEAD']);
        assertSourceCheckout(source, worktree);
        const refreshed = selection(repository, numbers);
        const manifest = captureTrain({ repository, base, source, directory: worktree, selections: refreshed });
        if (manifest.branch !== selected.branch || api(`${basePath(repository)}/git/ref/heads/dev`).object?.sha !== base) {
          fail('Train inputs changed during assembly');
        }
        const encoded = `${JSON.stringify(manifest, null, 2)}\n`;
        if (Buffer.byteLength(encoded) > 65536) fail('Train manifest exceeds bound');
        await writeFile(output, encoded, { flag: 'wx', mode: 0o600 });
        return manifest;
      } catch (error) {
        throw new Error(`Train assembly stopped; retained ${selected.branch} at ${worktree}: ${error.message}`);
      }
    },
    async capture(repository, source, numbers, output) {
      basePath(repository);
      assertSourceCheckout(source, directory);
      const base = api(`${basePath(repository)}/git/ref/heads/dev`).object?.sha;
      const manifest = captureTrain({ repository, base, source,
        selections: selection(repository, numbers), directory });
      // Refuse stale snapshots before publication; final landing must refresh
      // them again because an observation is not a compare-and-swap operation.
      if (api(`${basePath(repository)}/git/ref/heads/dev`).object?.sha !== base) fail('Development changed during capture');
      const encoded = `${JSON.stringify(manifest, null, 2)}\n`;
      if (Buffer.byteLength(encoded) > 65536) fail('Train manifest exceeds bound');
      await writeFile(output, encoded, { flag: 'wx', mode: 0o600 });
      return manifest;
    },
    async verify(filename, runId, products) {
      const manifest = await loadManifest(filename);
      const base = basePath(manifest.repository);
      // Reproduce tree identity locally; a rewritten JSON digest is not proof
      // that a claimed tree belongs to the workflow's actual source commit.
      if (!/^[a-f0-9]{40}$/.test(manifest.source ?? '') ||
          invoke('git', ['rev-parse', '--verify', `${manifest.source}^{tree}`]) !== manifest.tree) fail('Train source tree mismatch');
      const observation = {
        development: api(`${base}/git/ref/heads/dev`).object?.sha,
        selections: selection(manifest.repository, manifest.inputs?.map(p => p.number)),
        run: api(`${base}/actions/runs/${number(runId)}`),
      };
      const captured = captureTrain({ repository: manifest.repository, source: manifest.source,
        base: observation.development, selections: observation.selections, directory });
      if (captured.digest !== manifest.digest) fail('Train selection or reconstructed tree changed');
      return verifyTrainProducts(manifest, observation, products);
    },
  };
}
