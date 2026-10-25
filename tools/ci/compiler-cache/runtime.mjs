import { createHash, randomUUID } from 'node:crypto';
import { promises as fs } from 'node:fs';
import { spawn } from 'node:child_process';
import path from 'node:path';
import os from 'node:os';
import { performance } from 'node:perf_hooks';
import { CacheRefusal } from './store.mjs';

// Engine contracts: https://github.com/mozilla/sccache/blob/v0.19.0/docs/Rust.md
// https://github.com/mozilla/sccache/blob/v0.19.0/docs/Local.md
// Foreground custody uses SCCACHE_START_SERVER + SCCACHE_NO_DAEMON from
// pinned src/cmdline.rs and src/util.rs; this is deliberately version-specific.
export const SCCACHE_ENGINE = Object.freeze({
  version: '0.19.0',
  url: 'https://github.com/mozilla/sccache/releases/download/v0.19.0/sccache-v0.19.0-x86_64-unknown-linux-musl.tar.gz',
  archiveSha256: 'aa73c7270c997894038c0d8f0d297865f57569e1687883322bc431713d4bb9dd',
  binarySha256: '0b44834be857e21b026f7db49a7762023fedb5eb5ed60725a66f698d8d79a34b',
  member: 'sccache-v0.19.0-x86_64-unknown-linux-musl/sccache',
});
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const refuse = code => { throw new CacheRefusal(code); };
const sleep = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
async function canonicalDestination(value) {
  let existing = path.resolve(value); const missing = [];
  for (;;) {
    try { return path.join(await fs.realpath(existing), ...missing.reverse()); }
    catch (error) {
      if (error.code !== 'ENOENT') throw error;
      const parent = path.dirname(existing);
      if (parent === existing) throw error;
      missing.push(path.basename(existing)); existing = parent;
    }
  }
}
async function fileDigest(file, limit = 128 * 1024 * 1024) {
  const stat = await fs.lstat(file);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > limit) refuse('unsupported-input-file');
  return digest(await fs.readFile(file));
}
async function directoryBytes(root) {
  let total = 0; let count = 0;
  async function walk(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      if (++count > 100000) refuse('runtime-file-count-bound');
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) await walk(file);
      else if (entry.isFile()) total += (await fs.lstat(file)).size;
      else if (!entry.isSocket()) refuse('runtime-unsupported-file');
    }
  }
  await walk(root); return total;
}
async function run(command, args, options = {}) {
  const started = performance.now();
  return new Promise((resolve, reject) => {
    const { signal: abortSignal, ...spawnOptions } = options;
    const child = spawn(command, args, { ...spawnOptions, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
    const chunks = { stdout: [], stderr: [] }; let size = 0; let failure;
    const kill = () => { if (child.exitCode === null && child.signalCode === null) { try { process.kill(-child.pid, 'SIGKILL'); } catch {} } };
    const abort = () => { failure = abortSignal.reason ?? new CacheRefusal('runtime-cancelled'); kill(); };
    if (abortSignal?.aborted) abort(); else abortSignal?.addEventListener('abort', abort, { once: true });
    const timer = setTimeout(() => { failure = new CacheRefusal('runtime-command-timeout'); kill(); }, 120000);
    for (const name of ['stdout', 'stderr']) child[name].on('data', bytes => {
      size += bytes.length;
      if (size > 1024 * 1024) { failure = new CacheRefusal('runtime-log-bound'); kill(); }
      else chunks[name].push(bytes);
    });
    // Spawn failures still emit close: settle only there so listeners and
    // owned process cleanup finish before the caller releases its lease.
    child.on('error', error => { failure = error; });
    child.on('close', (code, signal) => {
      clearTimeout(timer);
      abortSignal?.removeEventListener('abort', abort);
      // Any automatically spawned fallback server remains in this owned group
      // because NO_DAEMON is set. Do not leave it behind on client completion.
      try { process.kill(-child.pid, 'SIGKILL'); } catch {}
      if (failure) return reject(failure);
      const result = { code, signal, wallMs: performance.now() - started, stdout: Buffer.concat(chunks.stdout).toString(), stderr: Buffer.concat(chunks.stderr).toString() };
      if (code !== 0) return reject(Object.assign(new CacheRefusal('runtime-command-failed'), { result }));
      resolve(result);
    });
  });
}

// Explicit task-local acquisition. No global PATH/config installation and no
// unauthenticated "latest" resolution: both archive and executable are pinned.
export async function acquirePinnedEngine(destination) {
  if (process.platform !== 'linux' || process.arch !== 'x64' || !path.isAbsolute(destination)) refuse('unsupported-engine-platform');
  await fs.mkdir(destination, { mode: 0o700 });
  const response = await fetch(SCCACHE_ENGINE.url, { signal: AbortSignal.timeout(60000) });
  if (!response.ok) refuse('engine-download-failed');
  let size = 0; const chunks = [];
  for await (const chunk of response.body) {
    size += chunk.length; if (size > 16 * 1024 * 1024) refuse('engine-download-bound'); chunks.push(chunk);
  }
  const archive = Buffer.concat(chunks);
  if (digest(archive) !== SCCACHE_ENGINE.archiveSha256) refuse('engine-archive-mismatch');
  const archivePath = path.join(destination, 'sccache.tar.gz');
  await fs.writeFile(archivePath, archive, { flag: 'wx', mode: 0o600 });
  // tar streams only the named member; no archive paths are extracted.
  const binary = path.join(destination, 'sccache');
  await new Promise((resolve, reject) => {
    const child = spawn('/usr/bin/tar', ['-xOzf', archivePath, SCCACHE_ENGINE.member], { stdio: ['ignore', 'pipe', 'pipe'] });
    const parts = []; let bytes = 0; let failure;
    child.stdout.on('data', chunk => { bytes += chunk.length; if (bytes > 32 * 1024 * 1024) { failure = new CacheRefusal('engine-extraction-bound'); child.kill(); } else parts.push(chunk); });
    child.stderr.resume(); child.on('error', reject);
    child.on('close', async code => {
      try {
        if (failure) throw failure;
        if (code !== 0) refuse('engine-extraction-failed');
        const executable = Buffer.concat(parts);
        if (digest(executable) !== SCCACHE_ENGINE.binarySha256) refuse('engine-binary-mismatch');
        await fs.writeFile(binary, executable, { flag: 'wx', mode: 0o700 }); resolve();
      } catch (error) { reject(error); }
    });
  });
  await fs.writeFile(path.join(destination, 'acquisition.json'), JSON.stringify(SCCACHE_ENGINE, null, 2), { flag: 'wx', mode: 0o600 });
  return binary;
}

// This API consumes a reviewed closure; it does not derive completeness from
// Cargo, Rust text or dep-info. In particular, external procedural-macro inputs
// are unsupported. The integration owner must establish the snapshot boundary.
export async function sealReviewedRustClosure({ source, inputs, compiler, review, externalProceduralMacros = false, immutableSnapshot = false }) {
  if (externalProceduralMacros || !immutableSnapshot || typeof review !== 'string' || !review.trim() || review.length > 4096) refuse('unsupported-input-custody');
  if (!path.isAbsolute(source) || !path.isAbsolute(compiler) || !Array.isArray(inputs) || !inputs.includes(source) || inputs.length > 4096 || new Set(inputs).size !== inputs.length || inputs.some(file => !path.isAbsolute(file))) refuse('unsupported-input-closure');
  const files = [];
  for (const file of [...inputs].sort()) files.push({ path: file, sha256: await fileDigest(file) });
  return { schema: 'reviewed-rust-closure-v1', source, compiler, compilerSha256: await fileDigest(compiler), files, review, externalProceduralMacros: false, immutableSnapshot: true };
}
async function verifyClosure(closure) {
  if (closure.schema !== 'reviewed-rust-closure-v1' || closure.externalProceduralMacros !== false || closure.immutableSnapshot !== true || typeof closure.review !== 'string' || !closure.review.trim() || closure.review.length > 4096 || !Array.isArray(closure.files) || closure.files.length > 4096 || !path.isAbsolute(closure.source ?? '') || !path.isAbsolute(closure.compiler ?? '') || !/^[a-f0-9]{64}$/.test(closure.compilerSha256) || !closure.files.some(file => file.path === closure.source) || closure.files.some(file => typeof file.path !== 'string' || !path.isAbsolute(file.path) || !/^[a-f0-9]{64}$/.test(file.sha256)) || new Set(closure.files.map(file => file.path)).size !== closure.files.length) refuse('unsupported-input-custody');
  if (await fileDigest(closure.compiler) !== closure.compilerSha256) refuse('compiler-custody-mismatch');
  for (const file of closure.files) if (await fileDigest(file.path) !== file.sha256) refuse('source-custody-mismatch');
}

// One owned foreground server per cache, for the entire session. Other
// controllers fail busy; a crash retains the lease for audited recovery.
// Cache files are engine-owned compiler objects; proof outputs stay task-local.
export async function withCompilerCache(options, action) {
  const { engine, maxCacheBytes, reserveBytes, maxTaskBytes } = options;
  let { cacheRoot, taskRoot } = options;
  if (![engine, cacheRoot, taskRoot].every(value => typeof value === 'string' && path.isAbsolute(value)) ||
      [maxCacheBytes, reserveBytes, maxTaskBytes].some(value => !Number.isSafeInteger(value) || value <= 0) ||
      cacheRoot === taskRoot || cacheRoot.startsWith(`${taskRoot}/`) || taskRoot.startsWith(`${cacheRoot}/`)) refuse('invalid-runtime-policy');
  [cacheRoot, taskRoot] = await Promise.all([canonicalDestination(cacheRoot), canonicalDestination(taskRoot)]);
  if (cacheRoot === taskRoot || cacheRoot.startsWith(`${taskRoot}/`) || taskRoot.startsWith(`${cacheRoot}/`)) refuse('invalid-runtime-policy');
  if (await fileDigest(engine) !== SCCACHE_ENGINE.binarySha256) refuse('engine-binary-mismatch');
  await fs.mkdir(cacheRoot, { recursive: true, mode: 0o700 });
  if (!(await fs.lstat(cacheRoot)).isDirectory() || (await fs.lstat(cacheRoot)).isSymbolicLink()) refuse('unsafe-runtime-root');
  const lock = path.join(cacheRoot, '.runtime-lease');
  try { await fs.mkdir(lock, { mode: 0o700 }); }
  catch (error) { if (error.code === 'EEXIST') refuse('runtime-cache-busy'); throw error; }
  const cancellation = new AbortController();
  let server; let serverExited; let serverTerminal; let socketRoot; let monitor; let monitorFailure; let active = true; let monitoring = false;
  const env = { PATH: '/usr/bin:/bin', LANG: 'C', HOME: taskRoot, XDG_CONFIG_HOME: taskRoot, TMPDIR: path.join(taskRoot, 'tmp'), CARGO_INCREMENTAL: '0', SCCACHE_NO_DAEMON: '1', SCCACHE_DIR: path.join(cacheRoot, 'objects'), SCCACHE_CACHE_SIZE: String(maxCacheBytes), SCCACHE_IDLE_TIMEOUT: '0', SCCACHE_DIRECT: 'false', SCCACHE_CONF: path.join(taskRoot, 'config.toml') };
  const check = async () => {
    if (serverTerminal) refuse(serverTerminal.error ? 'runtime-server-start-failed' : 'runtime-server-exited');
    const stat = await fs.statfs(cacheRoot, { bigint: true });
    if (stat.bavail * stat.bsize < BigInt(reserveBytes)) refuse('runtime-reserve-lost');
    if (await directoryBytes(taskRoot) > maxTaskBytes) refuse('runtime-task-bound');
    if (await directoryBytes(env.SCCACHE_DIR) > maxCacheBytes) refuse('runtime-cache-bound');
  };
  const command = async (args, cwd = taskRoot) => {
    if (!active) refuse('runtime-session-closed');
    if (monitorFailure) throw monitorFailure;
    await check();
    if (serverTerminal) refuse(serverTerminal.error ? 'runtime-server-start-failed' : 'runtime-server-exited');
    const result = await run(engine, args, { env, cwd, signal: cancellation.signal });
    await check(); if (monitorFailure) throw monitorFailure; return result;
  };
  try {
    await fs.writeFile(path.join(lock, 'owner.json'), JSON.stringify({ pid: process.pid, procStat: await fs.readFile(`/proc/${process.pid}/stat`, 'utf8'), bootId: (await fs.readFile('/proc/sys/kernel/random/boot_id', 'utf8')).trim() }), { flag: 'wx', mode: 0o600 });
    const marker = path.join(cacheRoot, 'policy.json');
    const policy = JSON.stringify({ schema: 1, engineSha256: SCCACHE_ENGINE.binarySha256, maxCacheBytes });
    try { if (await fs.readFile(marker, 'utf8') !== policy) refuse('runtime-policy-mismatch'); }
    catch (error) {
      if (error.code !== 'ENOENT') throw error;
      if ((await fs.readdir(cacheRoot)).some(name => name !== '.runtime-lease')) refuse('unmanaged-runtime-cache');
      await fs.writeFile(marker, policy, { flag: 'wx', mode: 0o600 });
    }
    if ((await fs.readdir(cacheRoot)).some(name => !['.runtime-lease', 'objects', 'policy.json'].includes(name))) refuse('unmanaged-runtime-cache');
    await fs.mkdir(env.SCCACHE_DIR, { recursive: true, mode: 0o700 });
    if ((await fs.lstat(env.SCCACHE_DIR)).isSymbolicLink()) refuse('unsafe-runtime-root');
    const stat = await fs.statfs(cacheRoot, { bigint: true });
    if (stat.bavail * stat.bsize < BigInt(reserveBytes) + BigInt(maxCacheBytes) + BigInt(maxTaskBytes)) refuse('runtime-reserve-admission');
    await fs.mkdir(taskRoot, { mode: 0o700 });
    await fs.mkdir(env.TMPDIR, { mode: 0o700 });
    await fs.writeFile(env.SCCACHE_CONF, '[cache.disk]\ndir = '+JSON.stringify(env.SCCACHE_DIR)+'\nsize = '+maxCacheBytes+'\n', { flag: 'wx', mode: 0o600 });
    socketRoot = await fs.mkdtemp(path.join(os.tmpdir(), 'conduit-sccache-'));
    env.SCCACHE_SERVER_UDS = path.join(socketRoot, 'server');
    server = spawn(engine, [], { env: { ...env, SCCACHE_START_SERVER: '1' }, cwd: taskRoot, detached: true, stdio: ['ignore', 'ignore', 'ignore'] });
    serverExited = new Promise(resolve => {
      server.once('error', error => { serverTerminal = { error }; });
      server.once('exit', (code, signal) => { serverTerminal = { code, signal }; });
      server.once('close', resolve);
    });
    for (let attempt = 0; attempt < 100; attempt++) {
      if (serverTerminal) refuse(serverTerminal.error ? 'runtime-server-start-failed' : 'runtime-server-exited');
      try { await fs.lstat(env.SCCACHE_SERVER_UDS); break; }
      catch (error) { if (error.code !== 'ENOENT') throw error; if (attempt === 99) refuse('runtime-server-start-timeout'); await sleep(50); }
    }
    monitor = setInterval(() => {
      if (monitoring) return; monitoring = true;
      check().catch(error => { monitorFailure = error; cancellation.abort(error); if (!serverTerminal) { try { process.kill(-server.pid, 'SIGKILL'); } catch {} } }).finally(() => { monitoring = false; });
    }, 250);
    const session = {
      stats: async () => JSON.parse((await command(['--show-stats', '--stats-format', 'json'])).stdout),
      compileRlib: async ({ closure, crateName }) => {
        if (!active) refuse('runtime-session-closed');
        if (!/^[a-z][a-z0-9_]{0,63}$/.test(crateName)) refuse('invalid-crate-name');
        closure = structuredClone(closure);
        await verifyClosure(closure);
        const version = await run(closure.compiler, ['-vV'], { env, cwd: taskRoot, signal: cancellation.signal });
        const output = path.join(taskRoot, `compile-${randomUUID()}`); await fs.mkdir(output);
        const args = [closure.compiler, closure.source, '--crate-name', crateName, '--crate-type=rlib', '--emit=link,metadata,dep-info', '--out-dir', output, '-C', 'opt-level=1'];
        const result = await command(args, path.dirname(closure.source)); await verifyClosure(closure);
        const files = [];
        for (const name of (await fs.readdir(output)).sort()) files.push({ name, sha256: await fileDigest(path.join(output, name)), bytes: (await fs.stat(path.join(output, name))).size });
        return { closureSha256: digest(JSON.stringify(closure)), compilerVersion: version.stdout, environment: { ...env }, args, wallMs: result.wallMs, files, stdout: result.stdout, stderr: result.stderr };
      },
    };
    return await action(session);
  } finally {
    active = false; clearInterval(monitor); cancellation.abort(new CacheRefusal('runtime-session-closed'));
    if (server) {
      if (!serverTerminal) {
        if (!monitorFailure) { try { await run(engine, ['--stop-server'], { env, cwd: taskRoot }); } catch {} }
        for (let attempt = 0; !serverTerminal && attempt < 40; attempt++) await sleep(50);
        if (!serverTerminal) { try { process.kill(-server.pid, 'SIGKILL'); } catch {} }
      }
      await serverExited;
    }
    while (monitoring) await sleep(10);
    if (socketRoot) await fs.rm(socketRoot, { recursive: true });
    await fs.rm(lock, { recursive: true });
  }
}
