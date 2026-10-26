import test from 'node:test';
import assert from 'node:assert/strict';
import { setupHostedSandbox } from '../../tools/ci/pipeline/targets/hosted-sandbox.mjs';
function fixture({ restricted = true, profile = '/etc/apparmor.d/bwrap-userns-restrict', failure } = {}) {
  const calls = [], records = [];
  const services = { exists: file => file.startsWith('/proc/') ? restricted : file === profile,
    read: () => '1\n', hash: () => 'a'.repeat(64), record: (...args) => records.push(args),
    run: (program, args, options) => { calls.push({ program, args, options }); if (program === failure) throw new Error('probe failure'); } };
  return { calls, records, services };
}
test('restricted runner loads only distribution profile then proves isolated namespace setup', () => {
  const f = fixture(), result = setupHostedSandbox(f.services);
  assert.deepEqual(f.calls[0].args, ['apparmor_parser', '-r', '/etc/apparmor.d/bwrap-userns-restrict']);
  assert.equal(f.calls[1].program, '/usr/bin/bwrap');
  assert.ok(f.calls[1].args.includes('--unshare-all'));
  assert.equal(result.isolatedNamespaceProbe, 'passed'); assert.equal(f.records.length, 1);
});
test('distribution extra-profile location is supported without copying or rewriting it', () => {
  const f = fixture({ profile: '/usr/share/apparmor/extra-profiles/bwrap-userns-restrict' });
  assert.equal(setupHostedSandbox(f.services).profile, '/usr/share/apparmor/extra-profiles/bwrap-userns-restrict');
});
test('unrestricted runner needs no privileged profile operation', () => {
  const f = fixture({ restricted: false }); setupHostedSandbox(f.services);
  assert.equal(f.calls.length, 1); assert.equal(f.calls[0].program, '/usr/bin/bwrap');
});
for (const failure of ['sudo', '/usr/bin/bwrap']) test(`failed ${failure} refuses without a successful tool receipt`, () => {
  const f = fixture({ failure }); assert.throws(() => setupHostedSandbox(f.services), /probe failure/);
  assert.equal(f.records.length, 0);
});
test('missing distribution profile refuses before any privileged operation or build', () => {
  const f = fixture({ profile: null }); assert.throws(() => setupHostedSandbox(f.services), /distribution/);
  assert.equal(f.calls.length, 0); assert.equal(f.records.length, 0);
});
