import { recordOperation } from '../acquisition/metrics.mjs';
import { acquireApt } from '../acquisition/apt.mjs';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, copyFileSync, mkdirSync, openSync, readSync, statSync } from 'node:fs';
import path from 'node:path';

export const toolchain = () => process.env.RUSTUP_TOOLCHAIN || 'stable';
export function command(program, args, options = {}) {
  console.log(`> ${program} ${args.join(' ')}`);
  const started = performance.now();
  const result = spawnSync(program, args, {
    stdio: 'inherit', timeout: 45 * 60_000,
    env: { ...process.env, RUSTUP_TOOLCHAIN: toolchain() }, ...options,
  });
  recordOperation({ kind: 'command', program, args, durationMs: performance.now() - started, outcome: result.error || result.status !== 0 ? 'failed' : 'success' });
  if (result.error || result.status !== 0) {
    throw new Error(`${program} ${args.join(' ')} failed: ${result.error?.message || result.status}`);
  }
  return result;
}
export const cargo = (...args) => command('cargo', [`+${toolchain()}`, ...args]);
// Keep the command first so the dependency-light dispatcher recognizes it.
export const xtask = (...args) => cargo('xtask', ...args, '--locked');

// Acquisition is the only retry boundary. Never wrap a build or a proof here.
export function acquire(program, args, options = {}) {
  try { return command(program, args, options); }
  catch (error) {
    console.warn(`Acquisition failed; one final attempt: ${error.message}`);
    return command(program, args, options);
  }
}
export { acquireApt as aptPackages } from '../acquisition/apt.mjs';
export function apt(...packages) { return acquireApt(packages); }
export function rustTargets(...targets) {
  acquire('rustup', ['target', 'add', '--toolchain', toolchain(), ...targets]);
}
export function rustComponents(...components) {
  acquire('rustup', ['component', 'add', '--toolchain', toolchain(), ...components]);
}
export function copyFile(source, destination) {
  if (!statSync(source).isFile() || statSync(source).size === 0) throw new Error(`Missing product: ${source}`);
  mkdirSync(path.dirname(destination), { recursive: true });
  copyFileSync(source, destination);
  if (digest(source) !== digest(destination)) throw new Error(`Product copy mismatch: ${destination}`);
}
export function digest(file) {
  const fd = openSync(file, 'r');
  try {
    const hash = createHash('sha256');
    const buffer = Buffer.alloc(1024 * 1024);
    for (;;) {
      const size = readSync(fd, buffer, 0, buffer.length, null);
      if (!size) return hash.digest('hex');
      hash.update(buffer.subarray(0, size));
    }
  } finally { closeSync(fd); }
}
export function assertDigest(file, expected) {
  if (digest(file) !== expected.replace(/^sha256:/, '')) throw new Error(`Artifact hash mismatch: ${file}`);
}
