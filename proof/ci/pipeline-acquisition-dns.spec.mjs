import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, appendFileSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { recoverUbuntuDns, ubuntuAddresses, ubuntuDnsFailure } from '../../tools/ci/pipeline/acquisition/ubuntu-dns.mjs';

const answer = host => ({ Status: 0, TC: false, CD: false,
  Question: [{ name: `${host}.`, type: 1 }],
  Answer: [{ name: `${host}.`, type: 1, TTL: 60, data: '192.0.2.1' }] });

test('HTTPS DNS answers must bind an allowed question and bounded IPv4 records', () => {
  const host = 'archive.ubuntu.com';
  assert.deepEqual(ubuntuAddresses(JSON.stringify(answer(host)), host), ['192.0.2.1']);
  for (const alter of [
    x => { x.Status = 2; }, x => { x.TC = true; }, x => { x.CD = true; },
    x => { x.Question[0].name = 'other.example.'; }, x => { x.Question[0].type = 28; },
    x => { x.Answer[0].name = 'other.example.'; }, x => { x.Answer[0].data = '192.0.2.1\nforged'; },
    x => { x.Answer[0].TTL = 0; }, x => { x.Answer = Array(17).fill(x.Answer[0]); },
  ]) {
    const value = answer(host); alter(value);
    assert.throws(() => ubuntuAddresses(JSON.stringify(value), host));
  }
  assert.throws(() => ubuntuAddresses(JSON.stringify(answer(host)), 'other.example'));
  assert.throws(() => ubuntuAddresses(' '.repeat(65_537), host));
  assert.equal(ubuntuDnsFailure(new Error("Temporary failure resolving 'archive.ubuntu.com'")), true);
  assert.equal(ubuntuDnsFailure(new Error('repository signature verification failed')), false);
});

function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-dns-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const file = path.join(root, 'hosts');
  writeFileSync(file, '127.0.0.1 localhost\n');
  const options = { enabled: true, hostsFile: file,
    run(program, args) {
      assert.equal(program, 'curl');
      assert.ok(args.includes('--fail'));
      const url = new URL(args.at(-1));
      assert.equal(url.origin, 'https://cloudflare-dns.com');
      return JSON.stringify(answer(url.searchParams.get('name')));
    },
    privileged(program, args, config) {
      assert.equal(program, 'tee');
      assert.equal(args.at(-1), file);
      if (args.includes('-a')) appendFileSync(file, config.input);
      else writeFileSync(file, config.input);
    },
  };
  return { root, file, options };
}

test('recovery removes only its own entries and preserves intervening host edits', t => {
  const { file, options } = fixture(t);
  const lease = recoverUbuntuDns(options);
  assert.match(readFileSync(file, 'utf8'), /192\.0\.2\.1 archive\.ubuntu\.com/);
  appendFileSync(file, '192.0.2.5 independent.example\n');
  lease.restore(); lease.restore();
  assert.equal(readFileSync(file, 'utf8'), '127.0.0.1 localhost\n192.0.2.5 independent.example\n');
});

test('invalid recovery or a non-regular host path cannot publish partial entries', t => {
  const { root, file, options } = fixture(t);
  const run = options.run;
  options.run = (program, args) => args.at(-1).includes('security.ubuntu.com') ? '{"Status":2}' : run(program, args);
  assert.throws(() => recoverUbuntuDns(options));
  assert.equal(readFileSync(file, 'utf8'), '127.0.0.1 localhost\n');
  const link = path.join(root, 'link'); symlinkSync(file, link);
  assert.throws(() => recoverUbuntuDns({ ...options, run, hostsFile: link }), /regular file/);
  assert.equal(recoverUbuntuDns({ enabled: false, run() { throw new Error('must not run'); } }), null);
  for (const env of [{}, { GITHUB_ACTIONS: 'true', RUNNER_ENVIRONMENT: 'self-hosted' }]) {
    assert.equal(recoverUbuntuDns({ env, run() { throw new Error('must not run'); } }), null);
  }
});

test('a failed partial append is rolled back before the failure escapes', t => {
  const { file, options } = fixture(t);
  const privileged = options.privileged;
  options.privileged = (program, args, config) => {
    if (args.includes('-a')) {
      appendFileSync(file, config.input.slice(0, 20));
      throw new Error('write failed');
    }
    return privileged(program, args, config);
  };
  assert.throws(() => recoverUbuntuDns(options), /write failed/);
  assert.equal(readFileSync(file, 'utf8'), '127.0.0.1 localhost\n');
});
