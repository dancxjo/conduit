import { spawnSync } from 'node:child_process';

// Ignored build/tool outputs may exist; every nonignored source must belong to
// the exact commit. Call again after proof, immediately before sealing.
export function assertSourceCheckout(sha, cwd = process.cwd(), environment) {
  function git(args) {
    const result = spawnSync('git', args, { cwd, encoding: 'utf8', ...(environment === undefined ? {} : { env: environment }) });
    if (result.error || result.status !== 0) {
      if (environment !== undefined) throw new Error('Source checkout check failed');
      throw new Error(`Source checkout check failed: git ${args.join(' ')}: ${result.error?.message || result.stderr || result.stdout}`);
    }
    return result.stdout;
  }
  if (git(['rev-parse', 'HEAD']).trim() !== sha) {
    throw new Error('Source checkout does not match source SHA');
  }
  git(['diff', '--exit-code', 'HEAD', '--']);
  const untracked = git(['ls-files', '--others', '--exclude-standard', '-z']).split('\0').filter(Boolean);
  if (untracked.length) {
    throw new Error(`Source checkout contains untracked files: ${untracked.map(name => JSON.stringify(name)).join(', ')}`);
  }
}
