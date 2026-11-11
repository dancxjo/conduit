/** Scoped transport recovery; HTTPS and Ubuntu package signatures remain authoritative. */
import { spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { lstatSync, readFileSync } from 'node:fs';
import { isIP } from 'node:net';

const hosts = ['archive.ubuntu.com', 'security.ubuntu.com'];
const endpoint = 'https://cloudflare-dns.com/dns-query';
export const ubuntuDnsFailure = error => /Temporary failure resolving ['"](?:archive|security)\.ubuntu\.com['"]/.test(error?.message ?? '');

export function ubuntuAddresses(raw, host) {
  if (!hosts.includes(host) || typeof raw !== 'string' || Buffer.byteLength(raw) > 65_536) throw new Error('Invalid Ubuntu DNS input');
  const wire = JSON.parse(raw);
  const name = value => typeof value === 'string' ? value.replace(/\.$/, '').toLowerCase() : '';
  if (wire.Status !== 0 || wire.TC !== false || wire.CD !== false ||
      !Array.isArray(wire.Question) || wire.Question.length !== 1 ||
      name(wire.Question[0].name) !== host || wire.Question[0].type !== 1 ||
      !Array.isArray(wire.Answer) || wire.Answer.length > 64) throw new Error('Ubuntu DNS response does not bind its question');
  const records = wire.Answer.filter(record => record.type === 1);
  if (!records.length || records.length > 16 || records.some(record =>
    name(record.name) !== host || isIP(record.data) !== 4 ||
    !Number.isSafeInteger(record.TTL) || record.TTL <= 0)) throw new Error('Invalid bounded Ubuntu address set');
  return [...new Set(records.map(record => record.data))];
}

function execute(program, args, { input } = {}) {
  const result = spawnSync(program, args, { input, encoding: 'utf8', timeout: 12_000, maxBuffer: 65_536 });
  if (result.error || result.status !== 0) throw new Error(`Ubuntu DNS recovery command failed: ${program}: ${result.error?.message ?? result.stderr}`);
  return result.stdout;
}

/** Recover only GitHub-hosted ephemeral Linux runners, never ordinary user hosts. */
export function recoverUbuntuDns(options = {}) {
  const env = options.env ?? process.env;
  const enabled = options.enabled ?? (env.GITHUB_ACTIONS === 'true' && env.RUNNER_ENVIRONMENT === 'github-hosted' && process.platform === 'linux');
  if (!enabled) return null;
  const run = options.run ?? execute;
  const privileged = options.privileged ?? ((program, args, config) => process.getuid?.() === 0 ? run(program, args, config) : run('sudo', ['-n', program, ...args], config));
  const file = options.hostsFile ?? '/etc/hosts';
  const rows = hosts.flatMap(host => {
    // DNS only supplies routing. The original HTTPS names, signed repository
    // scope, fresh indices, package hashes and resolution checks are unchanged.
    const raw = run('curl', ['--silent', '--show-error', '--fail', '--max-time', '10',
      '--header', 'accept: application/dns-json', `${endpoint}?name=${host}&type=A&cd=0`]);
    return ubuntuAddresses(raw, host).map(address => `${address} ${host}`);
  });
  const id = randomUUID();
  const block = `\n# BEGIN Conduit Ubuntu DNS ${id}\n${rows.join('\n')}\n# END Conduit Ubuntu DNS ${id}\n`;
  const assertFile = () => { if (!lstatSync(file).isFile()) throw new Error('Ubuntu DNS hosts path is not a regular file'); };
  assertFile();
  const original = readFileSync(file, 'utf8');
  let restored = false;
  const restore = () => {
    if (restored) return;
    assertFile();
    const current = readFileSync(file, 'utf8');
    const at = current.indexOf(block);
    let next;
    if (at !== -1) {
      if (current.lastIndexOf(block) !== at) throw new Error('Duplicate Ubuntu DNS recovery block');
      next = current.slice(0, at) + current.slice(at + block.length);
    } else if (current.startsWith(original) && block.startsWith(current.slice(original.length))) {
      next = original; // Roll back a failed partial append, without guessing at other edits.
    } else if (!current.includes(id)) {
      restored = true; // Another owner already removed our complete block.
      return;
    } else {
      throw new Error('Cannot safely remove incomplete Ubuntu DNS recovery block');
    }
    privileged('tee', [file], { input: next });
    restored = true;
  };
  try {
    privileged('tee', ['-a', file], { input: block });
  } catch (error) {
    restore();
    throw error;
  }
  return { restore, identity: { schema: 'conduit.ci/ubuntu-dns-recovery@1', provider: endpoint, hosts: [...hosts] } };
}
