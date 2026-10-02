import { existsSync, mkdirSync } from 'node:fs';
import { homedir } from 'node:os';
import path from 'node:path';
import { acquire, apt, command, rustComponents, rustTargets, toolchain, xtask } from './common.mjs';

export function setup(target) {
  if (process.platform === 'linux') apt('build-essential', 'pkg-config', 'libssl-dev', 'libasound2-dev', 'libudev-dev', 'cmake', 'libclang-dev');
  if (target.family === 'browser') {
    rustTargets('wasm32-unknown-unknown');
    acquire('npm', ['ci', '--prefix', 'proof/browser']);
    acquire('node', ['proof/browser/node_modules/@playwright/test/cli.js', 'install', '--with-deps', 'chromium']);
  } else if (target.family === 'conduitos') {
    const arch = target.id.slice('conduitos-'.length);
    const triples = {
      x86_64: ['x86_64-unknown-none'], ia32: ['i686-unknown-uefi', 'i686-unknown-linux-gnu'],
      aarch64: ['aarch64-unknown-none'], riscv64: ['riscv64gc-unknown-none-elf'], loongarch64: ['loongarch64-unknown-none'],
    };
    rustTargets(...triples[arch]);
    rustComponents('rust-src', 'llvm-tools-preview');
    // Install only the selected image's emulator and firmware carrier. The
    // LoongArch runner uses Ubuntu 26, which does not publish ovmf-ia32.
    const packages = {
      x86_64: ['qemu-system-x86'],
      ia32: ['qemu-system-x86', 'ovmf-ia32'],
      aarch64: ['qemu-system-arm', 'qemu-efi-aarch64'],
      riscv64: ['qemu-system-misc', 'u-boot-qemu'],
      // The verifier acquires and hash-checks its pinned LoongArch UEFI ROM.
      loongarch64: ['qemu-system-misc'],
    };
    apt('curl', 'tar', 'xorriso', ...packages[arch]);
  } else if (target.family === 'esp32') {
    if (target.id === 'esp32-c3') {
      acquire('rustup', ['toolchain', 'install', '1.91.1', '--profile', 'minimal', '--component', 'rust-src', '--target', 'riscv32imc-unknown-none-elf']);
    } else {
      mkdirSync('target', { recursive: true });
      acquire('cargo', [`+${toolchain()}`, 'install', 'espup', '--version', '0.16.0', '--locked']);
      acquire('espup', ['install', '--name', 'esp-conduit-1.91.1', '--toolchain-version', '1.91.1.0',
        '--crosstool-toolchain-version', '15.2.0_20250920',
        '--targets', target.id === 'esp32-wroom' ? 'esp32' : 'esp32s3',
        '--export-file', path.resolve('target/pipeline-esp-export.sh')]);
      activateXtensa();
    }
  } else if (target.id === 'avr') {
    // This entrance provisions and verifies tools only; it does not build firmware.
    xtask('make', 'avr', 'check');
  } else if (target.id === 'raspberry-pi') {
    rustComponents('rust-src', 'llvm-tools-preview');
    apt('dosfstools', 'mtools');
  } else if (target.id === 'orange-pi') {
    rustTargets('aarch64-unknown-none');
    rustComponents('rust-src', 'llvm-tools-preview');
    apt('dosfstools');
  } else if (target.id === 'rp2040') {
    rustTargets('thumbv6m-none-eabi');
    acquire('cargo', [`+${toolchain()}`, 'install', 'elf2uf2-rs', '--version', '2.2.0', '--locked']);
    // Hash-checking acquisition has no physical side effects or firmware build.
    xtask('make', 'pico', '--refresh-radio-assets');
  }
}

export function activateXtensa() {
  const root = path.join(process.env.RUSTUP_HOME || path.join(homedir(), '.rustup'), 'toolchains/esp-conduit-1.91.1');
  const gcc = path.join(root, 'xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/bin');
  const clang = path.join(root, 'xtensa-esp32-elf-clang/esp-20.1.1_20250829/esp-clang/lib');
  if (!existsSync(path.join(gcc, 'xtensa-esp32-elf-gcc')) || !existsSync(clang)) {
    throw new Error('Pinned Xtensa GCC/Clang layout unavailable; no firmware proof has run');
  }
  process.env.PATH = `${gcc}${path.delimiter}${process.env.PATH}`;
  process.env.LIBCLANG_PATH = clang;
  command('rustc', ['+esp-conduit-1.91.1', '--version']);
}
