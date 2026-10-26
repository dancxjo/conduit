import { existsSync, readFileSync } from 'node:fs';
import { command, digest } from './common.mjs';
import { recordTool } from '../acquisition/metrics.mjs';

// Ubuntu can restrict unprivileged namespace setup until its distribution
// Bubblewrap profile is loaded. Keep the global restriction and our complete
// namespace boundary; never fall back to building outside the sandbox.
export function setupHostedSandbox({ exists = existsSync, read = file => readFileSync(file, 'utf8'),
  run = command, hash = digest, record = recordTool } = {}) {
  const restriction = '/proc/sys/kernel/apparmor_restrict_unprivileged_userns';
  const restricted = exists(restriction) && read(restriction).trim() === '1';
  let profile = null;
  if (restricted) {
    profile = ['/etc/apparmor.d/bwrap-userns-restrict',
      '/usr/share/apparmor/extra-profiles/bwrap-userns-restrict'].find(exists);
    if (!profile) throw new Error('Restricted hosted namespaces require the distribution bwrap-userns-restrict profile');
    run('sudo', ['apparmor_parser', '-r', profile], { timeout: 15000 });
  }
  run('/usr/bin/bwrap', ['--unshare-all', '--die-with-parent', '--new-session', '--cap-drop', 'ALL',
    '--clearenv', '--ro-bind', '/', '/', '--proc', '/proc', '--dev', '/dev', '/usr/bin/true'], { timeout: 15000 });
  const identity = { restrictedUserNamespaces: restricted, profile, profileSha256: profile ? hash(profile) : null,
    bwrapSha256: hash('/usr/bin/bwrap'), isolatedNamespaceProbe: 'passed' };
  record('hosted-sandbox', identity);
  return identity;
}
