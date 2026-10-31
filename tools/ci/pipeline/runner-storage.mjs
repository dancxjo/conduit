// Unit/admission proofs use Rust, native libraries and Node, not these image SDKs.
// Never reclaim source, Rust caches, test output or tools on a user's machine.
import { lstatSync, realpathSync, statfsSync } from 'node:fs';
import { command } from './targets/common.mjs';

const UNUSED_IMAGE_SDKS = Object.freeze([
  '/usr/local/lib/android',
  '/usr/share/dotnet',
]);

export function prepareHostedUnitStorage({
  environment = process.env,
  platform = process.platform,
  inspect = lstatSync,
  canonicalPath = realpathSync,
  space = () => statfsSync('.'),
  run = command,
  log = console.log,
} = {}) {
  if (platform !== 'linux' || environment.GITHUB_ACTIONS !== 'true'
      || environment.RUNNER_ENVIRONMENT !== 'github-hosted') {
    return { skipped: true };
  }
  const free = () => {
    const disk = space();
    return disk.bavail * disk.bsize;
  };
  const beforeBytes = free();
  const removed = [];
  const preserved = [];
  for (const directory of UNUSED_IMAGE_SDKS) {
    let entry;
    try { entry = inspect(directory); }
    catch (error) {
      if (error.code === 'ENOENT') continue;
      throw error;
    }
    // Preserve unexpected files, symlinks and redirected ancestor paths.
    if (!entry.isDirectory() || entry.isSymbolicLink()
        || canonicalPath(directory) !== directory) {
      preserved.push(directory);
      continue;
    }
    removed.push(directory);
  }
  if (removed.length) {
    run('sudo', ['rm', '--recursive', '--force', '--one-file-system', '--', ...removed]);
  }
  const afterBytes = free();
  log(`Hosted unit storage: before=${beforeBytes} after=${afterBytes} removed=${removed.join(',')} preserved=${preserved.join(',')}`);
  return { skipped: false, beforeBytes, afterBytes, removed, preserved };
}
