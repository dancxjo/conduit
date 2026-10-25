import { execFileSync } from 'node:child_process';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { captureTrain } from '../../../tools/ci/pipeline/train.mjs';

const repository = 'example/conduit';
const repo = { full_name: repository };

export async function fixture(t) {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'conduit-train-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const git = (...args) => execFileSync('git', ['-C', directory, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  git('init', '-b', 'base');
  git('config', 'user.email', 'fixture@example.invalid');
  git('config', 'user.name', 'Train fixture');
  const commit = async name => {
    await writeFile(path.join(directory, name), name);
    git('add', name); git('commit', '-m', name);
    return git('rev-parse', 'HEAD');
  };
  const base = await commit('base');
  git('checkout', '-b', 'first'); const first = await commit('first');
  git('checkout', '-b', 'second', base); const second = await commit('second');
  git('checkout', '-b', 'assembled', first); git('merge', '--no-ff', second, '-m', 'assembled');
  const source = git('rev-parse', 'HEAD');
  const selections = [first, second].map((head, i) => ({
    pr: { number: i + 1, state: 'open', draft: false, mergeable: true,
      head: { sha: head, repo }, base: { ref: 'dev', sha: base, repo } },
    candidateRun: { repository: repo, head_repository: repo, event: 'pull_request',
      path: '.github/workflows/candidate.yml', head_sha: head, status: 'completed',
      conclusion: 'success', pull_requests: [{ number: i + 1 }] },
  }));
  const capture = overrides => captureTrain({ repository, base, selections, source, directory, ...overrides });
  const manifest = capture();
  const observation = { development: base, selections, run: { id: 123, repository: repo,
    head_repository: repo, path: '.github/workflows/integration.yml', event: 'workflow_dispatch',
    head_branch: manifest.branch, head_sha: source, status: 'completed', conclusion: 'success' } };
  return { directory, git, base, source, selections, capture, manifest, observation };
}
