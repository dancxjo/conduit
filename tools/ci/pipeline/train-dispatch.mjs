import { spawnSync } from 'node:child_process';
import { createTrainCommands } from './train-command.mjs';

const fail = message => { throw new Error(message); };

// The first creator of the exact remote train ref owns dispatch. An existing
// ref with no visible run is ambiguous, never permission to start duplicate CI.
export function createTrainDispatcher({ command = spawnSync, directory = process.cwd(), commands, observeCheapPolicy } = {}) {
  commands ??= createTrainCommands({ command, directory, observeCheapPolicy });
  const api = (endpoint, body) => {
    const args = ['api', endpoint];
    if (body) args.push('--method', 'POST', '--input', '-');
    const result = command('gh', args, { cwd: directory, encoding: 'utf8',
      maxBuffer: 2 * 1024 * 1024, ...(body ? { input: JSON.stringify(body) } : {}) });
    if (result.error || result.signal || result.status !== 0) fail('Train API failed; transport outcome unknown, inspect retained intent before retrying');
    return result.stdout.trim() ? JSON.parse(result.stdout) : null;
  };
  return async function dispatch(filename) {
    const manifest = await commands.prepareDispatch(filename);
    const base = `repos/${manifest.repository}`;
    const fullRef = `refs/heads/${manifest.branch}`;
    const response = api(`${base}/git/matching-refs/heads/${manifest.branch}?per_page=100`);
    if (!Array.isArray(response) || response.length > 100) fail('Invalid train ref inventory');
    const refs = response.filter(ref => ref.ref === fullRef);
    if (refs.length > 1 || (refs.length === 1 && refs[0].object?.sha !== manifest.source)) fail('Remote train ref differs from source');
    const inventory = api(`${base}/actions/workflows/integration.yml/runs?branch=${encodeURIComponent(manifest.branch)}&head_sha=${manifest.source}&event=workflow_dispatch&per_page=100`);
    if (!Array.isArray(inventory?.workflow_runs) || inventory.workflow_runs.length > 100) fail('Invalid train run inventory');
    const runs = inventory.workflow_runs.filter(run => run.head_sha === manifest.source && run.head_branch === manifest.branch);
    for (const run of runs) {
      if (!Number.isSafeInteger(run.id) || run.id <= 0 || run.path !== '.github/workflows/integration.yml' ||
          run.event !== 'workflow_dispatch' || run.repository?.full_name !== manifest.repository ||
          run.head_repository?.full_name !== manifest.repository) fail('Invalid exact-source train run');
    }
    const latest = runs.sort((a, b) => b.id - a.id)[0];
    if (latest) {
      if (refs.length !== 1) fail('Existing train run lacks its immutable ref');
      if (!['queued', 'in_progress', 'requested', 'waiting', 'pending', 'completed'].includes(latest.status) ||
          (latest.status === 'completed' && latest.conclusion !== 'success')) fail('Existing train proof failed; inspect its run before retrying');
      return { source: manifest.source, branch: manifest.branch, runId: latest.id, reused: true };
    }
    if (refs.length) fail('Train ref exists without a visible run; dispatch outcome requires inspection');
    const refreshed = await commands.prepareDispatch(filename);
    if (refreshed.digest !== manifest.digest) fail('Train changed before ref creation');
    // The merge exists only locally until this transfer. An empty expected-ref
    // lease allows creation alone: neither a changed nor identical existing ref
    // grants dispatch ownership. Git can report identical refs as up to date,
    // so require its exclusive new-branch acknowledgement as well as success.
    const pushed = command('git', ['-c', 'credential.helper=',
      '-c', 'credential.helper=!gh auth git-credential', '-c', 'credential.interactive=false',
      'push', '--porcelain', '--no-follow-tags', '--recurse-submodules=no', `--force-with-lease=${fullRef}:`,
      `https://github.com/${manifest.repository}.git`, `${manifest.source}:${fullRef}`], {
      cwd: directory, encoding: 'utf8', maxBuffer: 2 * 1024 * 1024, timeout: 120000,
      env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
    });
    if (pushed.error || pushed.signal || pushed.status !== 0) fail('Train ref push failed; transport outcome unknown, inspect retained intent before retrying');
    const acknowledgements = (pushed.stdout ?? '').split('\n').filter(line => /^[ =*!+\-]\t/.test(line));
    if (acknowledgements.length !== 1 || acknowledgements[0] !== `*\t${manifest.source}:${fullRef}\t[new branch]`) {
      fail('Exclusive new train ref creation was not acknowledged; inspect retained intent before retrying');
    }
    const current = api(`${base}/git/ref/heads/${manifest.branch}`);
    if (current?.object?.sha !== manifest.source) fail('Train ref moved before dispatch');
    // The workflow checks this expected source before scheduling expensive CI.
    // On a transport failure the retained ref is an ambiguous intent, not a retry token.
    api(`${base}/actions/workflows/integration.yml/dispatches`, {
      ref: manifest.branch, inputs: { 'train-source': manifest.source, 'train-manifest': JSON.stringify(manifest) },
    });
    return { source: manifest.source, branch: manifest.branch, runId: null,
      reused: false, dispatched: true, manifestDigest: manifest.digest };
  };
}
