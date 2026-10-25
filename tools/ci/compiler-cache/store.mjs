import { createHash, randomUUID } from 'node:crypto';
import { promises as fs } from 'node:fs';
import path from 'node:path';

const hash = value => createHash('sha256').update(value).digest('hex');
const idPattern = /^[a-f0-9]{64}-[a-f0-9]{64}$/;
export class CacheRefusal extends Error {
  constructor(code, message = code) { super(message); this.code = code; }
}
const refuse = code => { throw new CacheRefusal(code); };
const custodyFields = ['source', 'toolchain', 'features', 'test', 'environment'];
function custodyKey(custody) {
  if (!custody || Object.keys(custody).sort().join() !== [...custodyFields].sort().join()) refuse('invalid-custody');
  const canonical = JSON.stringify(custodyFields.map(key => {
    if (typeof custody[key] !== 'string' || !custody[key].trim() || custody[key].length > 4096) refuse('invalid-custody');
    return [key, custody[key]];
  }));
  if (Buffer.byteLength(canonical) > 8192) refuse('invalid-custody');
  return hash(canonical);
}
async function plainDirectory(directory) {
  const stat = await fs.lstat(directory);
  if (!stat.isDirectory() || stat.isSymbolicLink()) refuse('unsafe-directory');
}
async function plainFile(file) {
  const stat = await fs.lstat(file);
  if (!stat.isFile() || stat.nlink !== 1) refuse('unsafe-file');
  return stat;
}
async function readJson(file) {
  if ((await plainFile(file)).size > 16384) refuse('metadata-bound-exceeded');
  return JSON.parse(await fs.readFile(file, 'utf8'));
}
async function identity(pid = process.pid) {
  try {
    const boot = (await fs.readFile('/proc/sys/kernel/random/boot_id', 'utf8')).trim();
    const stat = await fs.readFile(`/proc/${pid}/stat`, 'utf8');
    return { pid, boot, start: stat.slice(stat.lastIndexOf(')') + 2).split(' ')[19] };
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
}
const defaultFreeBytes = async root => {
  const stat = await fs.statfs(root, { bigint: true });
  return stat.bavail * stat.bsize;
};

// Exclusive store operations and explicit leases protect compiler objects only.
// Task outputs and live Cargo target directories must never be placed here.
export class CompilerCache {
  constructor(root, policy, { freeBytes = defaultFreeBytes, clock = Date.now } = {}) {
    if (!path.isAbsolute(root)) refuse('absolute-root-required');
    for (const key of ['maxBytes', 'reserveBytes', 'maxObjectBytes', 'maxEntries']) {
      if (!Number.isSafeInteger(policy[key]) || policy[key] <= 0) refuse('invalid-policy');
    }
    if (policy.maxObjectBytes > policy.maxBytes) refuse('invalid-policy');
    this.root = root; this.policy = Object.fromEntries(['maxBytes', 'reserveBytes', 'maxObjectBytes', 'maxEntries'].map(key => [key, policy[key]])); this.freeBytes = freeBytes; this.clock = clock;
  }
  async initialize() {
    await fs.mkdir(this.root, { recursive: true, mode: 0o700 });
    await plainDirectory(this.root);
    return this.locked(async () => {
      const marker = path.join(this.root, 'policy.json');
      try {
        const saved = await readJson(marker);
        if (JSON.stringify(saved) !== JSON.stringify({ schema: 1, ...this.policy })) refuse('policy-mismatch');
      } catch (error) {
        if (error.code !== 'ENOENT') throw error;
        if ((await fs.readdir(this.root)).some(name => name !== '.lock')) refuse('nonempty-unmanaged-root');
        await fs.writeFile(marker, JSON.stringify({ schema: 1, ...this.policy }), { flag: 'wx', mode: 0o600 });
        await fs.mkdir(path.join(this.root, 'objects'), { mode: 0o700 });
        await fs.mkdir(path.join(this.root, 'leases'), { mode: 0o700 });
      }
      await plainDirectory(path.join(this.root, 'objects'));
      await plainDirectory(path.join(this.root, 'leases'));
    });
  }
  async locked(action) {
    await plainDirectory(this.root);
    const lock = path.join(this.root, '.lock');
    try { await fs.mkdir(lock, { mode: 0o700 }); }
    catch (error) { if (error.code === 'EEXIST') refuse('cache-busy'); throw error; }
    try {
      await fs.writeFile(path.join(lock, 'owner.json'), JSON.stringify(await identity()), { flag: 'wx', mode: 0o600 });
      return await action();
    } finally {
      await fs.unlink(path.join(lock, 'owner.json')).catch(error => { if (error.code !== 'ENOENT') throw error; });
      await fs.rmdir(lock);
    }
  }
  async inventory() {
    if ((await fs.readdir(this.root)).some(name => !['.lock', 'objects', 'leases', 'policy.json'].includes(name))) refuse('unmanaged-root-content');
    await plainDirectory(path.join(this.root, 'objects'));
    await plainDirectory(path.join(this.root, 'leases'));
    const names = await fs.readdir(path.join(this.root, 'objects'));
    if (names.length > this.policy.maxEntries) refuse('entry-bound-exceeded');
    const entries = [];
    for (const id of names) {
      if (!idPattern.test(id)) refuse('unknown-object');
      const directory = path.join(this.root, 'objects', id);
      await plainDirectory(directory);
      if ((await fs.readdir(directory)).sort().join() !== 'blob,manifest.json') refuse('unknown-object-content');
      const manifest = await readJson(path.join(directory, 'manifest.json'));
      const stat = await plainFile(path.join(directory, 'blob'));
      if (manifest.schema !== 1 || !Number.isSafeInteger(manifest.bytes) || manifest.bytes <= 0 ||
          manifest.bytes > this.policy.maxObjectBytes || stat.size !== manifest.bytes ||
          custodyKey(manifest.custody) !== id.slice(0, 64) || manifest.sha256 !== id.slice(65) ||
          !Number.isSafeInteger(manifest.usedAt)) refuse('corrupt-manifest');
      entries.push({ ...manifest, id, directory });
    }
    const leases = [];
    const leaseNames = await fs.readdir(path.join(this.root, 'leases'));
    if (leaseNames.length > this.policy.maxEntries) refuse('lease-bound-exceeded');
    for (const name of leaseNames) {
      if (!/^[a-f0-9-]{36}\.json$/.test(name)) refuse('unknown-lease');
      const lease = await readJson(path.join(this.root, 'leases', name));
      if (lease.schema !== 1 || !entries.some(entry => entry.id === lease.object) ||
          !Number.isSafeInteger(lease.owner?.pid) || lease.owner.pid <= 0 ||
          !/^[0-9]+$/.test(lease.owner?.start) || !/^[a-f0-9-]{36}$/.test(lease.owner?.boot)) refuse('corrupt-lease');
      leases.push({ ...lease, token: name.slice(0, -5) });
    }
    return { entries, leases };
  }
  async evictLocked(incomingBytes = 0, incomingEntries = 0, targetBytes = this.policy.maxBytes) {
    const { entries, leases } = await this.inventory();
    const protectedIds = new Set(leases.map(lease => lease.object));
    let bytes = entries.reduce((sum, entry) => sum + entry.bytes, 0);
    let count = entries.length;
    const removed = [];
    // Reserve includes bounded filesystem/manifest/lease overhead; payload quota
    // is separate. Recheck actual free bytes after every completed eviction.
    for (const entry of entries.sort((a, b) => a.usedAt - b.usedAt || a.id.localeCompare(b.id))) {
      const enough = bytes + incomingBytes <= targetBytes && count + incomingEntries <= this.policy.maxEntries &&
        BigInt(await this.freeBytes(this.root)) >= BigInt(this.policy.reserveBytes) + BigInt(incomingBytes) + 65536n;
      if (enough) break;
      if (protectedIds.has(entry.id)) continue;
      await fs.rm(entry.directory, { recursive: true });
      bytes -= entry.bytes; count--; removed.push(entry.id);
    }
    if (bytes + incomingBytes > targetBytes || count + incomingEntries > this.policy.maxEntries ||
        BigInt(await this.freeBytes(this.root)) < BigInt(this.policy.reserveBytes) + BigInt(incomingBytes) + 65536n) {
      throw new CacheRefusal('capacity-refused', JSON.stringify({ removed, retainedBytes: bytes }));
    }
    return { removed, retainedBytes: bytes };
  }
  async evict({ targetBytes = this.policy.maxBytes } = {}) {
    if (!Number.isSafeInteger(targetBytes) || targetBytes < 0 || targetBytes > this.policy.maxBytes) refuse('invalid-eviction-target');
    return this.locked(() => this.evictLocked(0, 0, targetBytes));
  }
  async put(custody, bytes) {
    const key = custodyKey(custody);
    custody = { ...custody };
    if (!Buffer.isBuffer(bytes) || !bytes.length || bytes.length > this.policy.maxObjectBytes) refuse('object-bound-exceeded');
    // Snapshot caller-owned bytes before hashing and async I/O.
    bytes = Buffer.from(bytes);
    const sha256 = hash(bytes); const id = `${key}-${sha256}`;
    return this.locked(async () => {
      const { entries } = await this.inventory();
      const existing = entries.find(entry => entry.id === id);
      if (existing) {
        if (hash(await fs.readFile(path.join(existing.directory, 'blob'))) !== sha256) refuse('corrupt-object');
        return { id, reused: true, evicted: [] };
      }
      const result = await this.evictLocked(bytes.length, 1);
      const temporary = path.join(this.root, `.incoming-${randomUUID()}`);
      await fs.mkdir(temporary, { mode: 0o700 });
      try {
        await fs.writeFile(path.join(temporary, 'blob'), bytes, { flag: 'wx', mode: 0o400 });
        await fs.writeFile(path.join(temporary, 'manifest.json'), JSON.stringify({ schema: 1, custody, sha256, bytes: bytes.length, usedAt: this.clock() }), { flag: 'wx', mode: 0o600 });
        if (BigInt(await this.freeBytes(this.root)) < BigInt(this.policy.reserveBytes)) refuse('reserve-lost');
        await fs.rename(temporary, path.join(this.root, 'objects', id));
      } finally { await fs.rm(temporary, { recursive: true, force: true }); }
      return { id, reused: false, evicted: result.removed };
    });
  }
  async acquire(custody, id) {
    if (!idPattern.test(id) || custodyKey(custody) !== id.slice(0, 64)) refuse('custody-mismatch');
    return this.locked(async () => {
      const { entries, leases } = await this.inventory();
      if (leases.length >= this.policy.maxEntries) refuse('lease-bound-exceeded');
      const entry = entries.find(candidate => candidate.id === id);
      if (!entry) refuse('cache-miss');
      const objectPath = path.join(entry.directory, 'blob');
      if (hash(await fs.readFile(objectPath)) !== entry.sha256) refuse('corrupt-object');
      if (BigInt(await this.freeBytes(this.root)) < BigInt(this.policy.reserveBytes) + 65536n) refuse('reserve-lost');
      const owner = await identity();
      if (!owner) refuse('process-identity-unavailable');
      const token = randomUUID();
      await fs.writeFile(path.join(this.root, 'leases', `${token}.json`), JSON.stringify({ schema: 1, owner, object: id }), { flag: 'wx', mode: 0o600 });
      const { directory: _directory, id: _id, ...manifest } = entry;
      await fs.writeFile(path.join(entry.directory, 'manifest.json'), JSON.stringify({ ...manifest, usedAt: this.clock() }));
      return { token, objectPath, sha256: entry.sha256, bytes: entry.bytes };
    });
  }
  async release(token, { reclaimDeadOwner = false } = {}) {
    if (!/^[a-f0-9-]{36}$/.test(token)) refuse('invalid-lease-token');
    return this.locked(async () => {
      const file = path.join(this.root, 'leases', `${token}.json`);
      const lease = (await this.inventory()).leases.find(candidate => candidate.token === token);
      if (!lease) refuse('lease-missing');
      const current = await identity();
      if (!current) refuse('process-identity-unavailable');
      if (!Number.isSafeInteger(lease.owner?.pid) || lease.owner.pid <= 0) refuse('corrupt-lease');
      const same = other => JSON.stringify(other) === JSON.stringify(lease.owner);
      if (!same(current)) {
        if (!reclaimDeadOwner || same(await identity(lease.owner.pid))) refuse('lease-owned');
      }
      await fs.unlink(file);
    });
  }
}
