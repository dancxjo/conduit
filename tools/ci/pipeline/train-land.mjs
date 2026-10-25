// Land only the exact source already proved and authenticated. Never force or bypass.
import { spawnSync } from 'node:child_process';
import { lstat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createTrainCommands } from './train-command.mjs';
import { verifyTrainManifest } from './train.mjs';

const fail = reason => { throw new Error(reason); };
const positive = value => Number.isSafeInteger(value) && value > 0;

export function validateLandingProtection(protection, rules) {
  if (!protection || !Array.isArray(rules) || rules.length) fail('Unrecognized branch rules prevent automatic landing');
  const status = protection.required_status_checks;
  if (!status || typeof status.strict !== 'boolean'
      || Object.keys(status).some(key => !['url', 'strict', 'contexts', 'contexts_url', 'checks'].includes(key))
      || !Array.isArray(status.contexts) || status.contexts.length !== 1
      || status.contexts[0] !== 'candidate' || !Array.isArray(status.checks) || status.checks.length !== 1
      || status.checks[0].context !== 'candidate' || status.checks[0].app_id !== 15368) fail('Unrecognized required checks');
  // Even a zero-review PR rule still requires a pull request. An admin token
  // must not silently turn a direct update into a policy bypass.
  if (protection.required_pull_request_reviews != null) fail('Pull request requirement prevents exact-source ref landing');
  if (protection.enforce_admins?.enabled !== true) fail('Required checks must apply to administrators before train landing');
  const harmless = new Set(['url', 'required_status_checks', 'required_pull_request_reviews', 'enforce_admins',
    'allow_force_pushes', 'allow_deletions', 'allow_fork_syncing', 'block_creations']);
  for (const [key, value] of Object.entries(protection)) {
    if (harmless.has(key)) continue;
    if (key === 'restrictions') {
      if (value != null) fail('Restricted push actors require separate admission');
    } else if (['required_linear_history', 'required_conversation_resolution', 'required_signatures', 'lock_branch'].includes(key)) {
      if (value?.enabled !== false) fail(`Unsupported branch requirement: ${key}`);
    } else fail(`Unknown branch protection: ${key}`);
  }
}

export function createTrainLander({ command = spawnSync, directory = process.cwd(), commands } = {}) {
  commands ??= createTrainCommands({ command, directory });
  const api = (endpoint, body) => {
    const args = ['api', endpoint];
    if (body) args.push('--method', 'PATCH', '--input', '-');
    const result = command('gh', args, { cwd: directory, encoding: 'utf8', timeout: 60000,
      maxBuffer: 2 * 1024 ** 2, ...(body ? { input: JSON.stringify(body) } : {}) });
    if (result.error || result.status !== 0) fail(body
      ? 'Landing mutation refused or ambiguous; inspect retained intent before any further action'
      : 'Landing observation failed');
    return JSON.parse(result.stdout);
  };
  return async function land(manifestFile, runId, archives, evidenceDirectory) {
    if (!path.isAbsolute(evidenceDirectory)) fail('Absolute fresh landing evidence directory required');
    try { await lstat(evidenceDirectory); fail('Landing evidence exists; inspect previous intent instead of retrying'); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    const manifest = await commands.prepareDispatch(manifestFile);
    verifyTrainManifest(manifest);
    if (!positive(Number(runId)) || String(Number(runId)) !== String(runId)) fail('Invalid Integration run ID');
    const base = `repos/${manifest.repository}`;
    const inspect = () => {
      const protection = api(`${base}/branches/dev/protection`);
      validateLandingProtection(protection, api(`${base}/rules/branches/dev`));
      const run = api(`${base}/actions/runs/${runId}`);
      if (run.id !== Number(runId) || !positive(run.run_attempt) || !positive(run.check_suite_id)
          || run.path !== '.github/workflows/integration.yml' || run.event !== 'workflow_dispatch'
          || run.head_branch !== manifest.branch || run.head_sha !== manifest.source
          || run.repository?.full_name !== manifest.repository || run.head_repository?.full_name !== manifest.repository
          || run.status !== 'completed' || run.conclusion !== 'success') fail('Exact successful train Integration required');
      const inventory = api(`${base}/actions/workflows/integration.yml/runs?branch=${encodeURIComponent(manifest.branch)}&head_sha=${manifest.source}&event=workflow_dispatch&per_page=100`);
      if (!Array.isArray(inventory.workflow_runs) || inventory.workflow_runs.length > 100
          || !Number.isSafeInteger(inventory.total_count) || inventory.total_count < 0
          || inventory.total_count > 100 || inventory.workflow_runs.length !== inventory.total_count) fail('Unbounded Integration run inventory');
      const matching = inventory.workflow_runs.filter(value => value.head_sha === manifest.source && value.head_branch === manifest.branch);
      if (matching.some(value => !positive(value.id))) fail('Invalid Integration run identity');
      const latest = matching.sort((a, b) => b.id - a.id)[0];
      if (!latest || latest.id !== run.id || latest.run_attempt !== run.run_attempt
          || latest.status !== 'completed' || latest.conclusion !== 'success') fail('Newer or changed Integration attempt');
      const checks = api(`${base}/check-suites/${run.check_suite_id}/check-runs?check_name=candidate&per_page=100`);
      if (checks.total_count !== 1 || checks.check_runs?.length !== 1) fail('Ambiguous required Candidate aggregate');
      const check = checks.check_runs[0];
      if (check.name !== 'candidate' || check.app?.id !== 15368 || check.head_sha !== manifest.source
          || check.check_suite?.id !== run.check_suite_id || check.status !== 'completed'
          || check.conclusion !== 'success') fail('Candidate aggregate does not belong to successful train proof');
      if (api(`${base}/git/ref/heads/${manifest.branch}`).object?.sha !== manifest.source) fail('Remote train ref changed');
      if (api(`${base}/git/commits/${manifest.source}`).tree?.sha !== manifest.tree) fail('Remote source tree differs');
      return run;
    };
    const before = inspect();
    const restored = await commands.restore(manifestFile, runId, archives, evidenceDirectory);
    if (restored.source !== manifest.source || restored.tree !== manifest.tree
        || restored.manifestDigest !== manifest.digest || restored.runId !== before.id
        || restored.runAttempt !== before.run_attempt) fail('Restored evidence differs from admitted train');
    const after = inspect();
    if (after.run_attempt !== before.run_attempt) fail('Integration attempt changed during restoration');
    const refreshed = await commands.prepareDispatch(manifestFile);
    if (refreshed.digest !== manifest.digest) fail('Train admission changed before landing');
    if (api(`${base}/git/ref/heads/dev`).object?.sha !== manifest.base) fail('Development changed before landing');
    const intent = { schema: 'conduit.train-landing-intent/v1', repository: manifest.repository,
      manifestDigest: manifest.digest, priorDev: manifest.base, source: manifest.source, tree: manifest.tree,
      runId: before.id, runAttempt: before.run_attempt, force: false };
    await writeFile(path.join(evidenceDirectory, 'landing-intent.json'), `${JSON.stringify(intent, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
    const updated = api(`${base}/git/refs/heads/dev`, { sha: manifest.source, force: false });
    if (updated.ref !== 'refs/heads/dev' || updated.object?.sha !== manifest.source) fail('Landing acknowledgement ambiguous; inspect retained intent');
    if (api(`${base}/git/ref/heads/dev`).object?.sha !== manifest.source
        || api(`${base}/git/commits/${manifest.source}`).tree?.sha !== manifest.tree) fail('Landing readback differs; inspect retained intent');
    const receipt = { ...intent, schema: 'conduit.train-landing/v1', landedDev: manifest.source,
      bundle: restored.bundle };
    await writeFile(path.join(evidenceDirectory, 'landing.json'), `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
    return receipt;
  };
}
