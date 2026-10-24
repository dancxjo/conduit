// Source retains small receipts and explicit regression/publication consumers.
// Historical raw run output belongs in external artifact storage.
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

export const ROOTS = ['docs/audits/evidence/', 'site/evidence/',
  'targets/conduitos/proof/firmware-frame/'];
export function checkEvidenceEntries(entries, policy) {
  const used = new Set();
  if (!policy?.retained || typeof policy.retained !== 'object') throw new Error('Missing evidence retention inventory');
  for (const root of ROOTS) {
    if (!Number.isSafeInteger(policy.budgets?.[root]) || policy.budgets[root] < 0)
      throw new Error(`Missing finite evidence budget: ${root}`);
    const files = entries.filter(entry => entry.path.startsWith(root));
    const bytes = files.reduce((sum, entry) => sum + entry.bytes.length, 0);
    if (bytes > policy.budgets[root]) throw new Error(`Evidence budget exceeded: ${root}`);
    for (const entry of files) {
      const raw = entry.bytes.length > 65536 || /\.(?:gz|xz|zip|tar|png|jpe?g|webp|wav|mp[34]|webm|iso|bin|elf|log)$/i.test(entry.path);
      if (!raw) continue;
      const retained = policy.retained[entry.path];
      const hash = createHash('sha256').update(entry.bytes).digest('hex');
      if (!retained?.consumer?.trim() || retained.sha256 !== hash || retained.bytes !== entry.bytes.length)
        throw new Error(`Raw evidence needs an exact named consumer or external archive: ${entry.path}`);
      used.add(entry.path);
    }
  }
  for (const name of Object.keys(policy.retained)) {
    if (!used.has(name)) throw new Error(`Remove stale evidence retention entry: ${name}`);
  }
}
export function checkEvidenceRetention(cwd = process.cwd()) {
  const result = spawnSync('git', ['ls-files', '-z', '--', ...ROOTS], { cwd, encoding: 'utf8' });
  if (result.error || result.status !== 0) throw new Error('Cannot inventory tracked evidence');
  const entries = result.stdout.split('\0').filter(Boolean).map(name => ({
    path: name, bytes: readFileSync(`${cwd}/${name}`),
  }));
  const policy = JSON.parse(readFileSync(`${cwd}/tools/ci/evidence-retention.json`, 'utf8'));
  checkEvidenceEntries(entries, policy);
}
