import test from 'node:test';
import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { CompilerCache } from '../../tools/ci/compiler-cache/store.mjs';

const custody = { source: 'sha256:source', toolchain: 'rustc-exact', features: 'a,b', test: 'compiler-object', environment: 'linux-x86_64' };
async function fixture(t, overrides = {}, options = {}) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'conduit-compiler-cache-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const cache = new CompilerCache(root, { maxBytes: 12, maxObjectBytes: 8, reserveBytes: 1024, maxEntries: 8, ...overrides }, { freeBytes: async () => 1000000n, ...options });
  await cache.initialize();
  return cache;
}
const rejects = (action, code) => assert.rejects(action, error => error.code === code);

test('immutable object deduplicates and exact custody is required', async t => {
  const cache = await fixture(t);
  const first = await cache.put(custody, Buffer.from('hello'));
  assert.equal((await cache.put(custody, Buffer.from('hello'))).reused, true);
  await rejects(() => cache.acquire({ ...custody, toolchain: 'other' }, first.id), 'custody-mismatch');
  const lease = await cache.acquire(custody, first.id);
  assert.equal(await fs.readFile(lease.objectPath, 'utf8'), 'hello');
  await cache.release(lease.token);
  assert.equal((await cache.inventory()).leases.length, 0);
});
test('deterministic eviction obeys byte quota and protects live readers', async t => {
  let time = 0;
  const cache = await fixture(t, {}, { clock: () => ++time });
  const first = await cache.put(custody, Buffer.from('aaaaaa'));
  const second = await cache.put(custody, Buffer.from('bbbbbb'));
  const lease = await cache.acquire(custody, first.id);
  const third = await cache.put(custody, Buffer.from('cccccc'));
  assert.deepEqual(third.evicted, [second.id]);
  await rejects(() => cache.acquire(custody, second.id), 'cache-miss');
  await cache.release(lease.token);
  assert.equal((await cache.inventory()).entries.reduce((sum, entry) => sum + entry.bytes, 0), 12);
});
test('all leased objects refuse admission without deleting active data', async t => {
  const cache = await fixture(t, { maxBytes: 8 });
  const object = await cache.put(custody, Buffer.from('12345678'));
  const lease = await cache.acquire(custody, object.id);
  await rejects(() => cache.put(custody, Buffer.from('abcdef')), 'capacity-refused');
  assert.equal(await fs.readFile(lease.objectPath, 'utf8'), '12345678');
  await cache.release(lease.token);
});
test('reserve admission fails before writing and cannot delete unrelated task evidence', async t => {
  let free = 1000000n;
  const cache = await fixture(t, {}, { freeBytes: async () => free });
  const evidence = path.join(path.dirname(cache.root), `${path.basename(cache.root)}-proof`);
  await fs.writeFile(evidence, 'unique proof'); t.after(() => fs.unlink(evidence));
  free = 0n;
  await rejects(() => cache.put(custody, Buffer.from('abc')), 'capacity-refused');
  assert.equal((await cache.inventory()).entries.length, 0);
  assert.equal(await fs.readFile(evidence, 'utf8'), 'unique proof');
});
test('reserve lost during writing removes only its unpublished temporary object', async t => {
  let calls = 0;
  const cache = await fixture(t, {}, { freeBytes: async () => ++calls <= 1 ? 1000000n : 0n });
  await rejects(() => cache.put(custody, Buffer.from('abc')), 'reserve-lost');
  assert.deepEqual((await fs.readdir(cache.root)).sort(), ['leases', 'objects', 'policy.json']);
});
test('object corruption fails closed before issuing a lease', async t => {
  const cache = await fixture(t);
  const object = await cache.put(custody, Buffer.from('abc'));
  const file = path.join(cache.root, 'objects', object.id, 'blob');
  await fs.chmod(file, 0o600); await fs.writeFile(file, 'xyz');
  await rejects(() => cache.acquire(custody, object.id), 'corrupt-object');
  assert.equal((await cache.inventory()).leases.length, 0);
});
test('exclusive writer lock is never automatically broken', async t => {
  const cache = await fixture(t);
  await fs.mkdir(path.join(cache.root, '.lock'));
  await rejects(() => cache.put(custody, Buffer.from('abc')), 'cache-busy');
  assert.equal((await cache.inventory()).entries.length, 0);
});
test('symlink objects and unmanaged roots are preserved and refused', async t => {
  const cache = await fixture(t);
  const id = `${'a'.repeat(64)}-${'b'.repeat(64)}`;
  await fs.symlink(os.tmpdir(), path.join(cache.root, 'objects', id));
  await rejects(() => cache.evict(), 'unsafe-directory');
  const unmanaged = await fs.mkdtemp(path.join(os.tmpdir(), 'conduit-unmanaged-'));
  t.after(() => fs.rm(unmanaged, { recursive: true, force: true }));
  await fs.writeFile(path.join(unmanaged, 'keep'), 'evidence');
  const other = new CompilerCache(unmanaged, cache.policy);
  await rejects(() => other.initialize(), 'nonempty-unmanaged-root');
  assert.equal(await fs.readFile(path.join(unmanaged, 'keep'), 'utf8'), 'evidence');
});
test('explicit dead-owner reclamation never steals a live lease', async t => {
  const cache = await fixture(t);
  const object = await cache.put(custody, Buffer.from('abc'));
  const lease = await cache.acquire(custody, object.id);
  const file = path.join(cache.root, 'leases', `${lease.token}.json`);
  const record = JSON.parse(await fs.readFile(file, 'utf8'));
  const child = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)']);
  t.after(() => child.kill());
  const boot = (await fs.readFile('/proc/sys/kernel/random/boot_id', 'utf8')).trim();
  const stat = await fs.readFile(`/proc/${child.pid}/stat`, 'utf8');
  record.owner = { pid: child.pid, boot, start: stat.slice(stat.lastIndexOf(')') + 2).split(' ')[19] };
  await fs.writeFile(file, JSON.stringify(record));
  await rejects(() => cache.release(lease.token, { reclaimDeadOwner: true }), 'lease-owned');
  const exited = new Promise(resolve => child.once('exit', resolve)); child.kill(); await exited;
  await cache.release(lease.token, { reclaimDeadOwner: true });
  assert.equal((await cache.inventory()).leases.length, 0);
});
test('policy mismatch, large custody and oversized objects refuse', async t => {
  const cache = await fixture(t);
  await rejects(() => new CompilerCache(cache.root, { ...cache.policy, maxBytes: 20 }).initialize(), 'policy-mismatch');
  await rejects(() => cache.put({ ...custody, source: 'x'.repeat(4097) }, Buffer.from('a')), 'invalid-custody');
  await rejects(() => cache.put(custody, Buffer.alloc(9)), 'object-bound-exceeded');
});
test('entry and lease counts are finite and explicit eviction has receipts', async t => {
  const cache = await fixture(t, { maxEntries: 1 });
  const first = await cache.put(custody, Buffer.from('abc'));
  const lease = await cache.acquire(custody, first.id);
  await rejects(() => cache.acquire(custody, first.id), 'lease-bound-exceeded');
  await rejects(() => cache.put(custody, Buffer.from('xyz')), 'capacity-refused');
  await cache.release(lease.token);
  const next = await cache.put(custody, Buffer.from('xyz'));
  assert.deepEqual(next.evicted, [first.id]);
  assert.equal((await cache.evict()).retainedBytes, 3);
});

test('explicit trim removes unleased objects and refuses active lease eviction', async t => {
  const cache = await fixture(t);
  const first = await cache.put(custody, Buffer.from('abc'));
  const second = await cache.put(custody, Buffer.from('xyz'));
  const lease = await cache.acquire(custody, first.id);
  await rejects(() => cache.evict({ targetBytes: 0 }), 'capacity-refused');
  assert.equal(await fs.readFile(lease.objectPath, 'utf8'), 'abc');
  await rejects(() => cache.acquire(custody, second.id), 'cache-miss');
  await cache.release(lease.token);
  const result = await cache.evict({ targetBytes: 0 });
  assert.deepEqual(result.removed, [first.id]);
  assert.equal(result.retainedBytes, 0);
  await rejects(() => cache.evict({ targetBytes: -1 }), 'invalid-eviction-target');
});

test('a live writer excludes concurrent operations until publication completes', async t => {
  let entered;
  const writing = new Promise(resolve => { entered = resolve; });
  let resume;
  const released = new Promise(resolve => { resume = resolve; });
  let paused = false;
  const cache = await fixture(t, {}, { freeBytes: async () => {
    if (!paused) { paused = true; entered(); await released; }
    return 1000000n;
  } });
  const pending = cache.put(custody, Buffer.from('abc'));
  await writing;
  try { await rejects(() => cache.put(custody, Buffer.from('xyz')), 'cache-busy'); }
  finally { resume(); }
  const completed = await pending;
  assert.equal((await cache.inventory()).entries[0].id, completed.id);
});
