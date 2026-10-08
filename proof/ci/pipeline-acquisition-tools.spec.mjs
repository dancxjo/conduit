import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir, homedir } from 'node:os';
import path from 'node:path';
import { ensureRust, cargoTool, cargoReceiptValid, parsePlaywrightDependencies, rustInstalled, ensureEspRiscv } from '../../tools/ci/pipeline/acquisition/tools.mjs';
import { targetPackages } from '../../tools/ci/pipeline/targets/setup.mjs';
import { measureAcquisition } from '../../tools/ci/pipeline/acquisition/metrics.mjs';
import { ACTIONLINT } from '../../tools/ci/pipeline/setup.mjs';

test('each ConduitOS package set includes only its own emulator/firmware', () => {
  const expected = {
    x86_64: ['qemu-system-x86'], ia32: ['qemu-system-x86', 'ovmf-ia32'],
    aarch64: ['qemu-system-arm', 'qemu-efi-aarch64'], riscv64: ['qemu-system-misc', 'u-boot-qemu'], loongarch64: ['qemu-system-misc'],
  };
  for (const [arch, selected] of Object.entries(expected)) {
    const packages = targetPackages({ family: 'conduitos', id: `conduitos-${arch}` });
    assert.deepEqual(packages.filter(name => /^(qemu|ovmf|u-boot)/.test(name)), selected);
    assert.equal(packages.length, new Set(packages).size);
    assert.ok(packages.includes('libssl-dev') && packages.includes('xorriso'));
    for (const dependency of ['libglib2.0-dev', 'libfdt-dev', 'zlib1g-dev', 'ninja-build', 'python3-venv', 'patch']) {
      assert.equal(packages.includes(dependency), arch === 'loongarch64', dependency);
    }
  }
  assert.throws(() => targetPackages({ family: 'conduitos', id: 'conduitos-unknown' }));
  assert.ok(targetPackages({ id: 'raspberry-pi' }).includes('mtools'));
  assert.ok(targetPackages({ id: 'orange-pi' }).includes('dosfstools'));
});
test('pinned Playwright dry-run discovers missing libraries without swallowing refusal', () => {
  assert.deepEqual(parsePlaywrightDependencies({ status: 0, stdout: 'All system dependencies are installed.\n' }), []);
  assert.deepEqual(parsePlaywrightDependencies({ status: 1, stdout: 'Missing system dependencies (2):\n  libasound2t64\n  fonts-liberation\n' }), ['libasound2t64', 'fonts-liberation']);
  for (const result of [{ status: 0, stdout: '' }, { status: 1, stdout: 'apt failed' }, { status: 1, stdout: 'Missing system dependencies (2):\n libx' }, { status: 1, stdout: 'Missing system dependencies (1):\n --help' }]) assert.throws(() => parsePlaywrightDependencies(result));
});
test('Rust installed checks use selected toolchain and exact targets', () => {
  const calls = [];
  const run = (program, args) => { calls.push([program, ...args]); return { stdout: 'rust-src\nllvm-tools-preview-x86_64-unknown-linux-gnu\n' }; };
  assert.deepEqual(rustInstalled('component', ['rust-src', 'llvm-tools-preview', 'clippy'], '1.98.1', run), ['clippy']);
  assert.deepEqual(calls[0], ['rustup', 'component', 'list', '--installed', '--toolchain', '1.98.1']);
  assert.deepEqual(rustInstalled('target', ['x86_64-unknown-none'], '1.98.1', () => ({ stdout: 'x86_64-unknown-none-other' })), ['x86_64-unknown-none']);
});
test('LLVM tools install alias recognizes rustup canonical installed component', () => {
  const run = () => ({ stdout: 'rust-src\nllvm-tools-x86_64-unknown-linux-gnu\n' });
  assert.deepEqual(rustInstalled('component', ['rust-src', 'llvm-tools-preview'], '1.98.1', run), []);
  ensureRust('component', ['llvm-tools-preview'], '1.98.1', run, () => assert.fail('installed LLVM tools must not be downloaded again'));
  assert.deepEqual(rustInstalled('target', ['llvm-tools-preview'], '1.98.1', run), ['llvm-tools-preview']);
});
const receipt = (version = '0.16.0') => ({ installs: { [`espup ${version} (registry+https://github.com/rust-lang/crates.io-index)`]: { bins: ['espup'] } } });
test('Cargo receipt rejects wrong version, source and executable', () => {
  assert.ok(cargoReceiptValid(receipt(), 'espup', '0.16.0', 'espup'));
  assert.equal(cargoReceiptValid(receipt('0.15.0'), 'espup', '0.16.0', 'espup'), false);
  assert.equal(cargoReceiptValid(receipt(), 'espup', '0.16.0', 'other'), false);
  assert.equal(cargoReceiptValid({ installs: { 'espup 0.16.0 (path+file:///tmp)': { bins: ['espup'] } } }, 'espup', '0.16.0', 'espup'), false);
});
test('warm Cargo tool verifies receipt and executable; stale binary is reacquired', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-tools-'));
  const priorPath = process.env.PATH;
  const priorGithub = process.env.GITHUB_PATH;
  delete process.env.GITHUB_PATH;
  try {
    const directory = path.join(root, 'cargo-tools/espup-0.16.0');
    mkdirSync(path.join(directory, 'bin'), { recursive: true });
    writeFileSync(path.join(directory, 'bin/espup'), 'fixture executable');
    writeFileSync(path.join(directory, '.crates2.json'), JSON.stringify(receipt()));
    let version = '0.16.0'; let installs = 0;
    const run = (_, args) => { assert.deepEqual(args, ['--version']); return { stdout: `espup ${version}\n` }; };
    const install = (program, args) => { installs++; assert.equal(program, 'cargo'); assert.ok(args.includes('=0.16.0') && args.includes('--locked') && args.includes('--root')); version = '0.16.0'; };
    cargoTool('espup', '0.16.0', { root, run, install });
    assert.equal(installs, 0);
    version = '0.15.0';
    cargoTool('espup', '0.16.0', { root, run, install });
    assert.equal(installs, 1);
    assert.equal(process.env.PATH.split(path.delimiter)[0], path.join(directory, 'bin'));
  } finally {
    process.env.PATH = priorPath;
    if (priorGithub !== undefined) process.env.GITHUB_PATH = priorGithub;
    rmSync(root, { recursive: true, force: true });
  }
});
test('warm ESP RISC-V reuses toolchain but installs missing exact target only', () => {
  const installs = [];
  ensureEspRiscv((program, args) => ({ stdout: program === 'rustc' ? 'rustc 1.91.1 (abc 2025-01-01)' : args[0] === 'toolchain' ? '1.91.1-x86_64-unknown-linux-gnu' : args[0] === 'component' ? 'rust-src' : installs.length ? 'riscv32imc-unknown-none-elf' : '' }), (_, args) => installs.push(args));
  assert.deepEqual(installs, [['target', 'add', '--toolchain', '1.91.1', 'riscv32imc-unknown-none-elf']]);
});
test('actionlint archive trust is checked in rather than fetched beside archive', () => {
  assert.equal(ACTIONLINT.sha256, '023070a287cd8cccd71515fedc843f1985bf96c436b7effaecce67290e7e0757');
});

test('Rust acquisition refuses successful installer that left required target absent', () => {
  assert.throws(() => ensureRust('target', ['wasm32-unknown-unknown'], '1.98.1', () => ({ stdout: '' }), () => {}), /incomplete/);
});

test('cold and warm Rust acquisition retain identical actual tool identities', async () => {
  const target = `spec-tools-stability-${process.pid}`;
  const reportPath = path.resolve(`target/acquisition/${target}.json`);
  const marker = path.join(homedir(), `.cache/conduit-ci/verified/${target}.json`);
  let installed = false;
  const acquire = () => ensureRust('target', ['wasm32-unknown-unknown'], 'fixture-pinned', () => ({ stdout: installed ? 'wasm32-unknown-unknown' : '' }), () => { installed = true; });
  try {
    await measureAcquisition(target, acquire);
    const cold = JSON.parse(readFileSync(reportPath, 'utf8'));
    await measureAcquisition(target, acquire);
    const warm = JSON.parse(readFileSync(reportPath, 'utf8'));
    assert.deepEqual(cold.tools, warm.tools);
    assert.equal(cold.operations.at(-1).cacheHit, false);
    assert.equal(warm.operations.at(-1).cacheHit, true);
  } finally { rmSync(reportPath, { force: true }); rmSync(marker, { force: true }); }
});
test('Pico setup verifies tracked radio assets instead of refreshing downloads', () => {
  const source = readFileSync(new URL('../../tools/ci/pipeline/targets/setup.mjs', import.meta.url), 'utf8');
  assert.match(source, /xtask\('make', 'pico', 'doctor'\)/);
  assert.doesNotMatch(source, /--refresh-radio-assets/);
});
