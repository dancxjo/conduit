// Read live train evidence through the repository's existing gh credentials.
// No merge, ref update, workflow dispatch or artifact publication occurs here.
import { spawnSync } from 'node:child_process';
import { lstat, readFile, writeFile } from 'node:fs/promises';
import { captureTrain, verifyTrainProducts } from './train.mjs';
import { assertSourceCheckout } from './source.mjs';

const fail = message => { throw new Error(message); };
const repositoryPattern = /^[\w.-]+\/[\w.-]+$/;
const number = value => {
  if (!/^[1-9][0-9]*$/.test(String(value)) || !Number.isSafeInteger(Number(value))) fail('Invalid numeric identity');
  return Number(value);
};

export function createTrainCommands({ command = spawnSync, directory = process.cwd() } = {}) {
  const invoke = (program, args) => {
    const result = command(program, args, { cwd: directory, encoding: 'utf8', maxBuffer: 2 * 1024 * 1024 });
    if (result.error || result.status !== 0) fail(`${program} failed: ${result.error?.message ?? result.stderr ?? result.status}`);
    return result.stdout.trim();
  };
  const api = endpoint => JSON.parse(invoke('gh', ['api', endpoint]));
  const basePath = repository => {
    if (typeof repository !== 'string' || !repositoryPattern.test(repository)) fail('Invalid repository');
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
      if (!Array.isArray(response.workflow_runs) || response.workflow_runs.length > 100) fail('Invalid Candidate run inventory');
      // Never select an older green run over a newer failed/running attempt.
      const matching = response.workflow_runs.filter(run => run.head_sha === pr.head.sha);
      for (const run of matching) number(run.id);
      const latest = matching.sort((a, b) => b.id - a.id)[0];
      if (!latest) fail('No exact-head Candidate run');
      return { pr, candidateRun: latest };
    });
  };
  return {
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
      const stat = await lstat(filename);
      if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 65536) fail('Invalid train manifest file');
      const manifest = JSON.parse(await readFile(filename, 'utf8'));
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
      return verifyTrainProducts(manifest, observation, products);
    },
  };
}
