import assert from 'node:assert/strict';
import { readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { command, digest } from '../targets/common.mjs';
import { recordTool } from './metrics.mjs';

const NIGHTLY = 'nightly-2024-07-22';
function output(program, args) {
  return command(program, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], timeout: 30_000 }).stdout.trim();
}

/** Called only after make avr check validates the retained installation content. */
export function recordAvrIdentities(root = process.cwd()) {
  const base = path.join(root, 'target/avr-promicro');
  const receiptPath = path.join(base, 'core-verification.json');
  assert(statSync(receiptPath).size <= 32 * 1024 * 1024, 'AVR receipt exceeds bound');
  const receipt = JSON.parse(readFileSync(receiptPath, 'utf8'));
  assert.equal(receipt.schema, 'conduit.avr-core-installation/v1');
  assert.equal(receipt.arduino, '1.8.8');
  assert.equal(receipt.sparkfun, '1.1.13');
  assert(receipt.files && Object.keys(receipt.files).length > 0, 'AVR receipt omitted installed content');
  recordTool('avr-core-installation', {
    arduino: receipt.arduino, sparkfun: receipt.sparkfun,
    sha256: digest(receiptPath), files: Object.keys(receipt.files).length,
  });

  const cli = path.join(base, 'tools/arduino-cli');
  const cliVersion = output(cli, ['version']);
  assert(/\bVersion:\s*1\.5\.1\b/.test(cliVersion), 'Arduino CLI version differs from pinned installation');
  recordTool('arduino-cli', { version: cliVersion, sha256: digest(cli) });

  const rustc = output('rustup', ['which', '--toolchain', NIGHTLY, 'rustc']);
  assert(path.isAbsolute(rustc), 'rustup must return the installed compiler path');
  const toolchain = path.dirname(path.dirname(rustc));
  const manifest = path.join(toolchain, 'lib/rustlib/multirust-channel-manifest.toml');
  assert(/^date\s*=\s*"2024-07-22"\s*$/m.test(readFileSync(manifest, 'utf8')), 'AVR nightly manifest date changed');
  const version = output(rustc, ['--version', '--verbose']);
  assert(/^rustc \S+-nightly\b/.test(version), 'AVR compiler is not nightly');
  recordTool('avr-rustc', {
    toolchain: NIGHTLY, version, sha256: digest(rustc), manifestSha256: digest(manifest),
    coreSourceSha256: digest(path.join(toolchain, 'lib/rustlib/src/rust/library/core/src/lib.rs')),
    allocSourceSha256: digest(path.join(toolchain, 'lib/rustlib/src/rust/library/alloc/src/lib.rs')),
  });
}
