// Unprivileged CI admission can schedule exhaustive proof, never dispatch,
// restore accepted artifacts, publish or land. No mutable-policy bypass option.
import { spawnSync } from 'node:child_process';
import { lstat, readFile } from 'node:fs/promises';
import { assertSourceCheckout } from './source.mjs';
import { verifyTrainManifest, verifyTrainProofAdmission } from './train.mjs';
import { readTrainSelections } from './train-observation.mjs';
const fail = reason => { throw new Error(`Train proof admission: ${reason}`); };

export function createTrainProofAdmission({ command = spawnSync, directory = process.cwd() } = {}) {
  const api = endpoint => {
    // All callers construct repository-scoped read endpoints. No --method,
    // request body, git push, credential helper or privileged observer exists.
    const result = command('gh', ['api', endpoint], { cwd: directory, encoding: 'utf8',
      timeout: 60000, maxBuffer: 2 * 1024 * 1024 });
    if (result.error || result.signal || result.status !== 0) fail('read-only API observation failed');
    return JSON.parse(result.stdout);
  };
  return async function admit(filename) {
    const stat = await lstat(filename);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size < 2 || stat.size > 65536) fail('bounded regular manifest required');
    const manifest = JSON.parse(await readFile(filename, 'utf8'));
    const value = verifyTrainManifest(manifest);
    assertSourceCheckout(value.source, directory);
    const observe = () => ({ development: api(`repos/${value.repository}/git/ref/heads/dev`).object?.sha,
      selections: readTrainSelections(value.repository, value.inputs.map(input => input.number), api) });
    const before = verifyTrainProofAdmission(manifest, observe(), directory);
    // Reobserve all PR/readiness/run/attempt/job custody after reconstruction.
    // A successful first observation is never a cache for mutable API state.
    const after = verifyTrainProofAdmission(manifest, observe(), directory);
    if (JSON.stringify(after) !== JSON.stringify(before)) fail('proof admission changed');
    assertSourceCheckout(value.source, directory);
    return after;
  };
}
