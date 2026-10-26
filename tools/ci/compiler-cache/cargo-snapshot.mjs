import { promises as fs, createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { CacheRefusal } from './refusal.mjs';
import { SCCACHE_ENGINE } from './engine.mjs';

const fail = code => { throw new CacheRefusal(code); };
const EXPECTED_MANIFEST = '[package]\nname = "conduitos-compositor-core"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n\n[lib]\npath = "src/lib.rs"\n';
export async function hashFile(file) {
  const hash = createHash('sha256');
  for await (const bytes of createReadStream(file)) hash.update(bytes);
  return hash.digest('hex');
}
export async function snapshotInventory(root, { maxBytes = 2 * 1024 ** 3, maxFiles = 4096 } = {}) {
  if(!Number.isSafeInteger(maxBytes)||maxBytes<=0||maxBytes>6*1024**3||!Number.isSafeInteger(maxFiles)||maxFiles<=0||maxFiles>150000)fail('invalid-cargo-snapshot-limits');
  const files = []; let bytes = 0;
  async function walk(relative = '') {
    for (const name of (await fs.readdir(path.join(root, relative))).sort()) {
      const item = path.posix.join(relative, name); const file = path.join(root, item);
      const stat = await fs.lstat(file);
      if (stat.isDirectory()) await walk(item);
      else if (stat.isFile() && stat.nlink === 1) {
        bytes += stat.size;
        if (bytes > maxBytes || files.length >= maxFiles) fail('cargo-snapshot-bound');
        files.push({ path: item, bytes: stat.size, executable: Boolean(stat.mode & 0o111), sha256: await hashFile(file) });
      } else fail('cargo-snapshot-unsupported-file');
    }
  }
  await walk();
  return { files, bytes, sha256: createHash('sha256').update(JSON.stringify(files)).digest('hex') };
}
export async function copyRegular(source, destination, executable = false) {
  const actual = await fs.realpath(source); const stat = await fs.lstat(actual);
  if (!stat.isFile() || stat.size > 512 * 1024 ** 2) fail('cargo-snapshot-unsupported-file');
  await fs.mkdir(path.dirname(destination), { recursive: true });
  await fs.copyFile(actual, destination, 1 /* COPYFILE_EXCL */);
  await fs.chmod(destination, executable ? 0o555 : 0o444);
  if (await hashFile(actual) !== await hashFile(destination)) fail('cargo-snapshot-copy-mismatch');
}
export async function assertSupportedPackage(source) {
  if (await fs.readFile(path.join(source, 'Cargo.toml'), 'utf8') !== EXPECTED_MANIFEST) fail('unsupported-cargo-package-contract');
  if ((await fs.readdir(source)).some(name => !['Cargo.toml', 'Cargo.lock', 'README.md', 'src', 'build.rs'].includes(name))) fail('unsupported-cargo-package-content');
  try { await fs.lstat(path.join(source, 'build.rs')); fail('unsupported-cargo-build-script'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
}

// Initial supported closure is the actual dependency-free compositor-core
// package, not a general Cargo resolver. No registry/vendor inputs exist here.
export async function prepareCompositorSnapshot({ packageRoot, toolchainRoot, engine, destination }) {
  if (![packageRoot, toolchainRoot, engine, destination].every(value => typeof value === 'string' && path.isAbsolute(value))) fail('cargo-absolute-path-required');
  packageRoot = await fs.realpath(packageRoot); toolchainRoot = await fs.realpath(toolchainRoot); engine = await fs.realpath(engine);
  const resolved = path.resolve(destination);
  destination = path.join(await fs.realpath(path.dirname(resolved)), path.basename(resolved));
  if ([packageRoot, toolchainRoot].some(root => destination === root || destination.startsWith(`${root}/`))) fail('overlapping-cargo-snapshot');
  let copiedBytes = 0, copiedFiles = 0;
  const copy = async (source, target, executable = false) => {
    const stat = await fs.stat(await fs.realpath(source));
    copiedBytes += stat.size; copiedFiles++;
    if (copiedBytes > 2 * 1024 ** 3 || copiedFiles > 4096) fail('cargo-snapshot-bound');
    await copyRegular(source, target, executable);
  };
  await assertSupportedPackage(packageRoot);
  if (await hashFile(engine) !== SCCACHE_ENGINE.binarySha256) fail('engine-binary-mismatch');
  const space = await fs.statfs(path.dirname(destination), { bigint: true });
  if (space.bavail * space.bsize < 3n * 1024n ** 3n) fail('cargo-snapshot-reserve');
  await fs.mkdir(destination, { mode: 0o700 });
  const source = path.join(destination, 'source');
  await fs.mkdir(source);
  async function copySource(relative = '') {
    for (const name of await fs.readdir(path.join(packageRoot, relative))) {
      const rel = path.join(relative, name); const file = path.join(packageRoot, rel); const stat = await fs.lstat(file);
      if (stat.isDirectory()) { await fs.mkdir(path.join(source, rel)); await copySource(rel); }
      else if (stat.isFile() && !['Cargo.lock'].includes(rel)) await copy(file, path.join(source, rel));
      else fail('cargo-source-unsupported-file');
    }
  }
  await copySource();
  // Exact standalone lockfile for this dependency-free package; Cargo --frozen
  // subsequently validates it. Original package sources/manifests are unchanged.
  await fs.writeFile(path.join(source, 'Cargo.lock'), '# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = "conduitos-compositor-core"\nversion = "0.1.0"\n', { flag: 'wx', mode: 0o444 });
  await copyCompilerClosure({ toolchainRoot, engine, destination, copy });
  const host = 'x86_64-unknown-linux-gnu';
  const inventory = await snapshotInventory(destination);
  // Symlink-free immutable copied snapshot: read-only bwrap mounts establish
  // the compiler boundary; hashes before/after detect host-side mutation.
  return { schema: 'compositor-cargo-snapshot-v1', root: destination, host, ...inventory, package: { name: 'conduitos-compositor-core', dependencies: [], buildScripts: [], proceduralMacros: [] } };
}

// Shared exact copied Rust/Cargo/runtime closure; no host directories are mounted.
export async function copyCompilerClosure({ toolchainRoot, engine, destination, copy, portableRuntime = false }) {
  for (const binary of ['cargo', 'rustc']) await copy(path.join(toolchainRoot, 'bin', binary), path.join(destination, 'toolchain/bin', binary), true);
  for (const name of await fs.readdir(path.join(toolchainRoot, 'lib'))) {
    if (name.startsWith('librustc_driver-') || name.startsWith('libLLVM')) await copy(path.join(toolchainRoot, 'lib', name), path.join(destination, 'toolchain/lib', name));
  }
  const host = 'x86_64-unknown-linux-gnu';
  const sysroot = `lib/rustlib/${host}/lib`;
  for (const name of await fs.readdir(path.join(toolchainRoot, sysroot))) await copy(path.join(toolchainRoot, sysroot, name), path.join(destination, 'toolchain', sysroot, name));
  // Complete dynamic dependencies of the selected official Rust/Cargo binaries
  // on this Linux x86_64 host; no host library directory is mounted wholesale.
  if (!portableRuntime) {
    for (const name of ['libdl.so.2', 'librt.so.1', 'libpthread.so.0', 'libc.so.6', 'libgcc_s.so.1', 'libm.so.6', 'libz.so.1', 'libsystemd.so.0', 'libselinux.so.1', 'libpcre2-8.so.0']) await copy(`/usr/lib/x86_64-linux-gnu/${name}`, path.join(destination, 'runtime/usr/lib/x86_64-linux-gnu', name));
    await copy('/lib64/ld-linux-x86-64.so.2', path.join(destination, 'runtime/usr/lib64/ld-linux-x86-64.so.2'), true);
  }
  for (const [input, output] of [['/usr/bin/dash','bin/sh'],['/usr/bin/sleep','usr/bin/sleep']]) await copy(input, path.join(destination, 'runtime', output), true);
  await copy(engine, path.join(destination, 'engine/sccache'), true);
}
