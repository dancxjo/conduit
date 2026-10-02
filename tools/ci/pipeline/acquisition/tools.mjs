// Reusable tool acquisition. Cached tools are verified before entering PATH.
import { appendFileSync, mkdirSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { acquire, command, digest, toolchain } from '../targets/common.mjs';
import { recordOperation, recordTool } from './metrics.mjs';

export const COMMON_APT = ['build-essential', 'pkg-config', 'libssl-dev', 'libasound2-dev', 'libudev-dev', 'cmake', 'libclang-dev'];
export const cacheRoot = () => path.join(homedir(), '.cache/conduit-ci');
const output = { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] };
export function exposePath(directory) {
  if (!process.env.PATH?.split(path.delimiter).includes(directory)) process.env.PATH = `${directory}${path.delimiter}${process.env.PATH || ''}`;
  if (process.env.GITHUB_PATH) appendFileSync(process.env.GITHUB_PATH, `${directory}\n`);
}
export function rustInstalled(kind, requested, selected = toolchain(), run = command) {
  const installed = run('rustup', [kind, 'list', '--installed', '--toolchain', selected], output).stdout.trim().split(/\s+/);
  // rustup accepts the historical llvm-tools-preview install alias but lists
  // its canonical llvm-tools name. Target names still require exact matches.
  return requested.filter(name => {
    const names = kind === 'component' && name === 'llvm-tools-preview' ? [name, 'llvm-tools'] : [name];
    return !installed.some(item => names.some(candidate => item === candidate || (kind === 'component' && item.startsWith(`${candidate}-`))));
  });
}
export function ensureRust(kind, requested, selected = toolchain(), run = command, install = acquire) {
  const start = performance.now();
  const missing = rustInstalled(kind, requested, selected, run);
  if (missing.length) {
    install('rustup', [kind, 'add', '--toolchain', selected, ...missing]);
    if (rustInstalled(kind, requested, selected, run).length) throw new Error(`Rust ${kind} installation incomplete`);
  }
  recordTool(`rust-${kind}s-${selected}`, { toolchain: selected, requested: [...requested].sort() });
  recordOperation({ kind: 'cache', program: 'rustup', args: [kind, ...requested], durationMs: performance.now() - start, outcome: 'success', cacheHit: missing.length === 0, installed: missing });
}
export const rustTargets = (...targets) => ensureRust('target', targets);
export const rustComponents = (...components) => ensureRust('component', components);
export function cargoReceiptValid(receipt, crate, version, binary) {
  return Object.entries(receipt.installs || {}).some(([key, value]) =>
    key === `${crate} ${version} (registry+https://github.com/rust-lang/crates.io-index)` && value.bins?.includes(binary));
}
export function cargoTool(crate, version, { binary = crate, reportsVersion = true, root = cacheRoot(), run = command, install = acquire } = {}) {
  const start = performance.now();
  const directory = path.join(root, 'cargo-tools', `${crate}-${version}`);
  const executable = path.join(directory, 'bin', `${binary}${process.platform === 'win32' ? '.exe' : ''}`);
  const valid = () => {
    try {
      if (!cargoReceiptValid(JSON.parse(readFileSync(path.join(directory, '.crates2.json'), 'utf8')), crate, version, binary)) return false;
      const result = run(executable, [reportsVersion ? '--version' : '--help'], output);
      // elf2uf2-rs 2.2.0 has no version flag: the exact Cargo registry receipt
      // owns its version; executing help still rejects an unusable cached binary.
      return reportsVersion ? result.stdout.trim().split(/\s+/)[1] === version : result.stdout.includes(`Usage: ${binary}`);
    } catch { return false; }
  };
  const warm = valid();
  if (!warm) {
    mkdirSync(directory, { recursive: true });
    install('cargo', [`+${toolchain()}`, 'install', crate, '--version', `=${version}`, '--locked', '--root', directory, '--force']);
    if (!valid()) throw new Error(`Installed ${crate} does not match pinned ${version}`);
  }
  recordTool(crate, { version, executableSha256: digest(executable) });
  recordOperation({ kind: 'cache', program: crate, args: [], durationMs: performance.now() - start, outcome: 'success', cacheHit: warm });
  exposePath(path.join(directory, 'bin'));
  return executable;
}
// Playwright 1.62's dry run uses APT simulation and reports every missing
// dependency. Do not maintain a second handwritten browser library list.
export function parsePlaywrightDependencies(result) {
  if (result.error) throw result.error;
  const text = result.stdout?.trim() || '';
  if (result.status === 0 && text === 'All system dependencies are installed.') return [];
  const match = /^Missing system dependencies \((\d+)\):\n([\s\S]+)$/.exec(text);
  if (result.status !== 1 || !match) throw new Error(`Playwright dependency discovery failed: ${result.stderr || text}`);
  const packages = match[2].trim().split(/\s+/);
  if (packages.length !== Number(match[1]) || packages.some(name => !/^[a-z0-9][a-z0-9+.:\-]*$/.test(name))) throw new Error('Invalid Playwright dependency report');
  return packages;
}
export function playwrightDependencies(run = spawnSync) {
  return parsePlaywrightDependencies(run('node', ['proof/browser/node_modules/@playwright/test/cli.js', 'install-deps', '--dry-run', 'chromium'], { ...output, timeout: 60_000 }));
}
export function ensureEspRiscv(run = command, install = acquire) {
  const selected = '1.91.1';
  const installed = run('rustup', ['toolchain', 'list'], output).stdout.split(/\s+/).some(name => name.startsWith(`${selected}-`));
  if (!installed) install('rustup', ['toolchain', 'install', selected, '--profile', 'minimal', '--component', 'rust-src', '--target', 'riscv32imc-unknown-none-elf']);
  ensureRust('component', ['rust-src'], selected, run, install);
  ensureRust('target', ['riscv32imc-unknown-none-elf'], selected, run, install);
  const version = run('rustc', [`+${selected}`, '--version', '--verbose'], output).stdout.trim();
  if (!version.startsWith('rustc 1.91.1 ')) throw new Error('Unexpected ESP RISC-V compiler version');
  recordTool('esp-riscv-rustc', { version });
}

export function verifyPlaywright({ root = path.resolve('proof/browser'), run = command } = {}) {
  const require = createRequire(path.join(root, 'package.json'));
  const core = path.dirname(require.resolve('playwright-core/package.json'));
  const version = JSON.parse(readFileSync(path.join(core, 'package.json'), 'utf8')).version;
  const lockPath = path.join(root, 'package-lock.json');
  const lock = JSON.parse(readFileSync(lockPath, 'utf8'));
  if (version !== lock.packages['node_modules/playwright-core'].version) throw new Error('Playwright installed version differs from lock');
  const manifest = JSON.parse(readFileSync(path.join(core, 'browsers.json'), 'utf8'));
  const registry = require('playwright-core/lib/coreBundle').registry.registry;
  recordTool('playwright', { version, lockSha256: digest(lockPath) });
  for (const name of ['chromium', 'chromium-headless-shell', 'ffmpeg']) {
    const descriptor = manifest.browsers.find(browser => browser.name === name);
    const executable = registry.findExecutable(name).executablePathOrDie('javascript');
    const actual = run(executable, [name === 'ffmpeg' ? '-version' : '--version'], output).stdout.trim().split('\n')[0];
    if (descriptor.browserVersion && !actual.split(/\s+/).includes(descriptor.browserVersion)) throw new Error(`${name} installed version differs from pinned revision`);
    recordTool(`playwright-${name}`, { revision: descriptor.revision, version: actual, executableSha256: digest(executable) });
  }
}
