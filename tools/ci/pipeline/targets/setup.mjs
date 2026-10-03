import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import path from 'node:path';
import { acquire, apt, command, digest, xtask } from './common.mjs';

import { COMMON_APT, cacheRoot, cargoTool, ensureEspRiscv, exposePath, playwrightDependencies, rustComponents, rustTargets, verifyPlaywright } from '../acquisition/tools.mjs';
import { recordOperation, recordTool } from '../acquisition/metrics.mjs';
import { recordAvrIdentities } from '../acquisition/avr-identities.mjs';

export function targetPackages(target) {
  const packages = [...COMMON_APT];
  if (target.family === 'conduitos') {
    const byArch = {
      x86_64: ['qemu-system-x86'], ia32: ['qemu-system-x86', 'ovmf-ia32'],
      aarch64: ['qemu-system-arm', 'qemu-efi-aarch64'],
      riscv64: ['qemu-system-misc', 'u-boot-qemu'], loongarch64: ['qemu-system-misc'],
    };
    const required = byArch[target.id.slice('conduitos-'.length)];
    if (!required) throw new Error(`Unknown ConduitOS architecture: ${target.id}`);
    packages.push('curl', 'tar', 'xorriso', ...required);
  } else if (target.id === 'raspberry-pi') packages.push('dosfstools', 'mtools');
  else if (target.id === 'orange-pi') packages.push('dosfstools');
  return packages;
}

// espup can leave its named toolchain partially installed after a download failure.
// A plain second install then fails on files from the first attempt. The name is
// dedicated to this pipeline, so use espup's own uninstall before one retry.
export function acquireXtensa(espup, args, { run = command, verify = activateXtensa } = {}) {
  const install = () => { run(espup, args); verify(); };
  try { install(); }
  catch (error) {
    console.warn(`Xtensa acquisition or verification failed; cleaning the pipeline toolchain before one final attempt: ${error.message}`);
    run(espup, ['uninstall', '--name', 'esp-conduit-1.91.1']);
    install();
  }
}

export function setup(target) {
  // Resolve the pinned browser package before deriving its platform libraries.
  if (target.family === 'browser') acquire('npm', ['ci', '--prefix', 'proof/browser']);
  if (process.platform === 'linux') apt(...targetPackages(target), ...(target.family === 'browser' ? playwrightDependencies() : []));
  if (target.family === 'browser') {
    rustTargets('wasm32-unknown-unknown');
    acquire('node', ['proof/browser/node_modules/@playwright/test/cli.js', 'install', 'chromium']);
    verifyPlaywright();
  } else if (target.family === 'conduitos') {
    const arch = target.id.slice('conduitos-'.length);
    const triples = {
      x86_64: ['x86_64-unknown-none'], ia32: ['i686-unknown-uefi', 'i686-unknown-linux-gnu'],
      aarch64: ['aarch64-unknown-none'], riscv64: ['riscv64gc-unknown-none-elf'], loongarch64: ['loongarch64-unknown-none'],
    };
    rustTargets(...triples[arch]);
    rustComponents('rust-src', 'llvm-tools-preview');
  } else if (target.family === 'esp32') {
    if (target.id === 'esp32-c3') {
      ensureEspRiscv();
    } else {
      mkdirSync('target', { recursive: true });
      const started = performance.now();
      const espup = cargoTool('espup', '0.16.0');
      const selected = target.id === 'esp32-wroom' ? 'esp32' : 'esp32s3';
      const receipt = path.join(cacheRoot(), `esp-${selected}.json`);
      const identity = { rust: '1.91.1.0', gcc: '15.2.0_20250920', clang: '20.1.1_20250829', target: selected };
      let warm = false;
      try { warm = JSON.stringify(JSON.parse(readFileSync(receipt, 'utf8'))) === JSON.stringify(identity); if (warm) activateXtensa(); } catch { warm = false; }
      if (!warm) {
        acquireXtensa(espup, ['install', '--name', 'esp-conduit-1.91.1', '--toolchain-version', identity.rust,
          '--crosstool-toolchain-version', identity.gcc, '--targets', selected,
          '--export-file', path.resolve('target/pipeline-esp-export.sh')]);
        mkdirSync(path.dirname(receipt), { recursive: true });
        writeFileSync(receipt, JSON.stringify(identity));
      }
      recordTool('esp-toolchain', identity);
      recordOperation({ kind: 'cache', program: 'espup', args: [selected], durationMs: performance.now() - started, outcome: 'success', cacheHit: warm });
    }
  } else if (target.id === 'avr') {
    // This entrance provisions and verifies tools only; it does not build firmware.
    xtask('make', 'avr', 'check');
    recordAvrIdentities();
  } else if (target.id === 'raspberry-pi') {
    rustComponents('rust-src', 'llvm-tools-preview');
  } else if (target.id === 'orange-pi') {
    rustTargets('aarch64-unknown-none');
    rustComponents('rust-src', 'llvm-tools-preview');
  } else if (target.id === 'rp2040') {
    rustTargets('thumbv6m-none-eabi');
    cargoTool('elf2uf2-rs', '2.2.0', { reportsVersion: false });
    // The radio assets are tracked: doctor verifies their pinned hashes and tools
    // without redownloading, building firmware, or touching hardware.
    xtask('make', 'pico', 'doctor');
    recordTool('pico-radio-contract', { sha256: digest('targets/rp2040/firmware/pico-w-signal/make/xtask/doctor.rs') });
  }
}

export function activateXtensa() {
  const root = path.join(process.env.RUSTUP_HOME || path.join(homedir(), '.rustup'), 'toolchains/esp-conduit-1.91.1');
  const gcc = path.join(root, 'xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/bin');
  const clang = path.join(root, 'xtensa-esp32-elf-clang/esp-20.1.1_20250829/esp-clang/lib');
  if (!existsSync(path.join(gcc, 'xtensa-esp32-elf-gcc')) || !existsSync(clang)) {
    throw new Error('Pinned Xtensa GCC/Clang layout unavailable; no firmware proof has run');
  }
  exposePath(gcc);
  process.env.LIBCLANG_PATH = clang;
  const options = { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] };
  const rust = command('rustc', ['+esp-conduit-1.91.1', '--version'], options).stdout;
  const compiler = command(path.join(gcc, 'xtensa-esp-elf-gcc'), ['-dumpfullversion'], options).stdout.trim();
  recordTool('esp-xtensa-rustc', { version: rust.trim() });
  recordTool('esp-xtensa-gcc', { version: compiler });
  if (!/^rustc 1\.91\.1-nightly \([a-f0-9]+ \d{4}-\d{2}-\d{2}\) \(1\.91\.1\.0\)\s*$/.test(rust) || compiler !== '15.2.0') throw new Error('Pinned Xtensa compiler identity mismatch');
}
