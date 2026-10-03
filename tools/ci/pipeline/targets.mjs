import { cpSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { cargo, cargoArtifact, command, copyFile, digest, xtask } from './targets/common.mjs';
import { setup } from './targets/setup.mjs';
import { browser } from './targets/browser.mjs';
import { embedded } from './targets/embedded.mjs';

const lane = (id, family, runner, proofClass) => Object.freeze({ id, family, runner, proofClass });
export const TARGETS = Object.freeze([
  lane('browser', 'browser', 'ubuntu-24.04', 'browser'),
  lane('hosted-linux', 'hosted', 'ubuntu-24.04', 'executable'),
  lane('hosted-windows', 'hosted', 'windows-2025', 'executable'),
  lane('hosted-macos', 'hosted', 'macos-15', 'executable'),
  ...['x86_64', 'aarch64', 'ia32', 'riscv64', 'loongarch64'].map(arch =>
    lane(`conduitos-${arch}`, 'conduitos', arch === 'loongarch64' ? 'ubuntu-26.04' : 'ubuntu-24.04', 'emulator')),
  ...['c3', 's3', 'wroom'].map(chip => lane(`esp32-${chip}`, 'esp32', 'ubuntu-24.04', 'build')),
  ...['avr', 'raspberry-pi', 'orange-pi', 'rp2040'].map(id => lane(id, id, 'ubuntu-24.04', 'build')),
]);

function target(id) {
  const selected = TARGETS.find(item => item.id === id);
  if (!selected) throw new Error(`Unknown target: ${id}`);
  return selected;
}
export function setupTarget(id) { setup(target(id)); }

/** Return only after the retained product passes its declared proof class. */
export function runTarget(id, directory) {
  const selected = target(id);
  directory = path.resolve(directory);
  mkdirSync(directory, { recursive: true });
  if (readdirSync(directory).length) throw new Error(`Target output must be empty: ${directory}`);
  if (selected.family === 'browser') browser(directory);
  else if (selected.family === 'hosted') hosted(id, directory);
  else if (selected.family === 'conduitos') conduitos(id, directory);
  else embedded(id, directory);
}

function hosted(id, directory) {
  const platform = id.slice('hosted-'.length);
  const expected = { linux: 'linux', windows: 'win32', macos: 'darwin' }[platform];
  if (process.platform !== expected) throw new Error(`${id} requires its native runner`);
  // The old Linux release command also cross-builds Raspberry Pi. Keep this
  // native executable lane independent of every embedded compiler.
  cargo('build', '--locked', '--release', '-p', 'conduit', '--bin', 'conduit');
  const name = process.platform === 'win32' ? 'conduit.exe' : 'conduit';
  const binary = path.join(directory, name);
  copyFile(cargoArtifact('release', name), binary);
  const before = digest(binary);
  const report = path.join(directory, 'hello-report.json');
  const result = command(binary, ['run', 'plots/hello/main.conduit', '--await-terminal', '--report', report,
    '--artifacts', path.join(directory, 'hello-artifacts')], { stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', timeout: 60_000 });
  const output = result.stdout;
  process.stdout.write(output);
  if (!output.split(/\r?\n/).includes('HELLO, WORLD.') || !/^plan .+ complete$/m.test(output) ||
      !/^host std-host-1 boot boot-.+ profile rust-std protocol 1$/m.test(output)) {
    throw new Error('Retained executable did not complete the canonical hello plot');
  }
  const snapshot = JSON.parse(readFileSync(report, 'utf8'));
  if (snapshot.schema !== 'conduit.observatory.snapshot/v2' || snapshot.hosts.length !== 1 || snapshot.plans.length !== 1) {
    throw new Error('Retained executable produced an invalid runtime report');
  }
  if (digest(binary) !== before) throw new Error('Executable changed during proof');
  writeFileSync(path.join(directory, 'hello-stdout.txt'), output);
}

function conduitos(id, directory) {
  const arch = id.slice('conduitos-'.length);
  const profile = arch === 'x86_64' ? 'conduitos-native' : `conduitos-${arch}-${arch === 'ia32' ? 'pc' : 'virt'}`;
  xtask('make', 'host', 'build', `targets/conduitos/profiles/${profile}.host.conduit`, '--output', directory);
  xtask('make', 'host', 'verify', directory, '--boot');
  if (arch === 'x86_64') xtask('make', 'host', 'verify', directory, '--journey');
  xtask('make', 'host', 'verify', directory);
  const evidence = path.join(process.env.CONDUIT_CONDUITOS_TARGET_ROOT || 'target/conduitos', arch);
  mkdirSync(path.join(directory, 'evidence'));
  for (const name of readdirSync(evidence)) {
    if ((name.includes('-product-') && /\.(json|log|bin)$/.test(name)) || name === 'profile-built-boot.log') {
      copyFile(path.join(evidence, name), path.join(directory, 'evidence', name));
    }
  }
  if (!readdirSync(path.join(directory, 'evidence')).some(name => name.endsWith('-product-proof.json'))) {
    throw new Error(`${id} omitted its exact product boot proof`);
  }
  if (arch === 'x86_64') {
    for (const name of ['journey-proof.json', 'journey-serial.log', 'journey-qmp.log']) {
      copyFile(path.join(evidence, name), path.join(directory, 'evidence', name));
    }
    cpSync(path.join(evidence, 'journey-frames'), path.join(directory, 'evidence/journey-frames'),
      { recursive: true, errorOnExist: true, force: false });
  }
}
