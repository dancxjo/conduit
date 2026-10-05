/** CI build prerequisites resolve only from the signed Ubuntu archive. */
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';

export function aptScope(root, architecture) {
  const os = readFileSync(path.join(root, 'etc/os-release'), 'utf8');
  const fields = Object.fromEntries(os.split('\n').filter(line => /^[A-Z_]+=/.test(line))
    .map(line => { const at = line.indexOf('='); return [line.slice(0, at), line.slice(at + 1).replace(/^(["'])(.*)\1$/, '$2')]; }));
  const suite = { '24.04': 'noble', '26.04': 'resolute' }[fields.VERSION_ID];
  if (fields.ID !== 'ubuntu' || !suite || architecture !== 'amd64' ||
      (fields.VERSION_CODENAME && fields.VERSION_CODENAME !== suite)) {
    throw new Error('Unsupported CI APT profile: requires Ubuntu 24.04/noble or 26.04/resolute amd64');
  }
  const keyring = path.join(root, 'usr/share/keyrings/ubuntu-archive-keyring.gpg');
  const signingKeySha256 = createHash('sha256').update(readFileSync(keyring)).digest('hex');
  const stanza = (uri, suites) => `Types: deb\nURIs: ${uri}\nSuites: ${suites}\nComponents: main restricted universe multiverse\nArchitectures: amd64\nSigned-By: ${keyring}\n`;
  const source = `${stanza('https://archive.ubuntu.com/ubuntu', `${suite} ${suite}-updates`)}\n${stanza('https://security.ubuntu.com/ubuntu', `${suite}-security`)}`;
  return { policy: 'ubuntu-build-prerequisites@1', resolution: 'no-removal-no-version-downgrade@1', os, suite, source, signingKeySha256 };
}
