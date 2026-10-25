import { promises as fs } from 'node:fs';
import path from 'node:path';
import { withCompilerCache, sealReviewedRustClosure, SCCACHE_ENGINE } from './runtime.mjs';

// Internal proof helper. Root's xtask/CI composition owns the supported entrance.
export async function probeCompilerCache({ engine, compiler, root }) {
  await fs.mkdir(root, { mode: 0o700 });
  const source = path.join(root, 'input.rs');
  await fs.writeFile(source, 'pub fn triangular(n: u64) -> u64 { (0..n).sum() }\n', { flag: 'wx', mode: 0o400 });
  const closure = await sealReviewedRustClosure({ source, inputs: [source], compiler, review: 'Dependency-free single Rust file; no macros, attributes, modules, extern crates or filesystem effects.', immutableSnapshot: true });
  const cacheRoot = path.join(root, 'cache');
  const options = { engine, cacheRoot, maxCacheBytes: 16 * 1024 * 1024, maxTaskBytes: 16 * 1024 * 1024, reserveBytes: 1024 * 1024 * 1024 };
  const first = await withCompilerCache({ ...options, taskRoot: path.join(root, 'cold-task') }, async session => {
    const before = await session.stats();
    const compile = await session.compileRlib({ closure, crateName: 'conduit_compiler_cache_probe' });
    const after = await session.stats();
    return { before, compile, after };
  });
  const warm = await withCompilerCache({ ...options, taskRoot: path.join(root, 'warm-task') }, async session => {
    const before = await session.stats();
    const compile = await session.compileRlib({ closure, crateName: 'conduit_compiler_cache_probe' });
    const after = await session.stats();
    return { before, compile, after };
  });
  const count = (stats, key) => Object.values(stats.stats[key].counts).reduce((sum, value) => sum + value, 0);
  await fs.writeFile(path.join(root, 'observed-stats.json'), JSON.stringify({first,warm}, null, 2), {flag:'wx'});
  if (count(first.after, 'cache_misses') !== 1 || count(warm.after, 'cache_hits') !== 1) throw new Error('Actual engine did not report one cold miss and one warm hit');
  const artifact = run => run.compile.files.find(file => file.name.endsWith('.rlib'));
  if (artifact(first)?.sha256 !== artifact(warm)?.sha256) throw new Error('Cold/warm compiler artifacts differ');
  const receipt = { schema: 1, scope: 'Engine operation across two owned server sessions; no Conduit Cargo build or release proof', engine: SCCACHE_ENGINE, closure, cold: first, warm };
  await fs.writeFile(path.join(root, 'probe-receipt.json'), JSON.stringify(receipt, null, 2), { flag: 'wx', mode: 0o600 });
  return receipt;
}
