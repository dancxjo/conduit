import { createHash } from 'node:crypto';
import { appendFileSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

let active;
const slash = value => value.split(path.sep).join('/');
const safe = value => String(value || 'local').replace(/[^a-zA-Z0-9_.-]/g, '-');

function files(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const name = path.join(directory, entry.name);
    return entry.isDirectory() ? files(name) : [name];
  });
}

export function acquisitionIdentity(target, { root = process.cwd(), env = process.env, platform = process.platform, arch = process.arch, home = homedir() } = {}) {
  if (!/^[a-z0-9][a-z0-9_-]*$/.test(target)) throw new Error('Invalid acquisition target');
  const inputs = [
    ...files(path.join(root, 'tools/ci/pipeline/acquisition')),
    ...files(path.join(root, 'tools/xtask/src/commands/avr/avr_toolchain')),
    ...['tools/ci/pipeline/setup.mjs', 'tools/ci/pipeline/targets/setup.mjs', 'tools/ci/pipeline/targets/common.mjs',
      'tools/ci/pipeline/targets.mjs', 'proof/browser/package-lock.json', 'rust-toolchain.toml',
      'tools/xtask/src/commands/avr/avr_toolchain.rs', 'tools/xtask/src/commands/avr/rust_firmware.rs',
      'targets/rp2040/firmware/pico-w-signal/make/xtask/firmware.rs',
      'targets/rp2040/firmware/pico-w-signal/make/xtask/doctor.rs'].map(name => path.join(root, name)),
  ].filter(existsSync).sort();
  const hash = createHash('sha256');
  for (const file of inputs) {
    hash.update(slash(path.relative(root, file)) + '\0');
    hash.update(readFileSync(file));
    hash.update('\0');
  }
  hash.update(env.RUSTUP_TOOLCHAIN || 'stable');
  const specificationKey = hash.digest('hex');
  const runnerImage = { os: env.ImageOS || platform, version: env.ImageVersion || 'local' };
  const key = `conduit-acquisition-v1-${safe(platform)}-${safe(arch)}-${safe(runnerImage.os)}-${safe(runnerImage.version)}-${target}-${specificationKey}`;
  const paths = [path.join(home, '.cache/conduit-ci')];
  if (target === 'browser') paths.push(path.join(home, '.cache/ms-playwright'), path.join(home, '.npm'));
  const rustup = env.RUSTUP_HOME || path.join(home, '.rustup');
  if (['esp32-s3', 'esp32-wroom'].includes(target)) paths.push(path.join(rustup, 'toolchains/esp-conduit-1.91.1'));
  if (target === 'esp32-c3') paths.push(path.join(rustup, `toolchains/1.91.1-${arch === 'x64' ? 'x86_64' : arch}-unknown-linux-gnu`));
  if (target === 'avr') paths.push(path.join(rustup, 'toolchains/nightly-2024-07-22-x86_64-unknown-linux-gnu'),
    ...['tools', 'arduino/data', 'arduino/downloads', 'core-verification.json'].map(name => path.join(root, 'target/avr-promicro', name)));
  return { target, key, specificationKey, runnerImage, paths,
    cacheable: !['hosted-windows', 'hosted-macos'].includes(target) };
}

export function emitAcquisitionKey(target) {
  const identity = acquisitionIdentity(target);
  const text = `key=${identity.key}\ncacheable=${identity.cacheable}\npaths<<CONDUIT_ACQUISITION_PATHS\n${identity.paths.join('\n')}\nCONDUIT_ACQUISITION_PATHS\n`;
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, text);
  console.log(JSON.stringify(identity));
  return identity;
}

export function recordOperation(operation) {
  if (active) active.operations.push(operation);
}

export function recordTool(name, identity) {
  if (active) active.tools.set(name, { name, ...identity });
}

function version(program, args) {
  const result = spawnSync(program, args, { encoding: 'utf8', timeout: 30_000 });
  if (result.error || result.status !== 0) throw new Error(`Cannot verify ${program} identity`);
  return result.stdout.trim();
}

/** Reports acquisition, never product proof. Failed setup retains a failed report. */
export async function measureAcquisition(target, operation) {
  if (active) throw new Error('Nested acquisition measurement');
  const identity = acquisitionIdentity(target);
  const sourceSha = version('git', ['rev-parse', 'HEAD']);
  if (!/^[a-f0-9]{40}$/.test(sourceSha)) throw new Error('Invalid source identity');
  const started = performance.now();
  active = { operations: [], tools: new Map() };
  let outcome = 'failure';
  let failure;
  try {
    recordTool('node', { version: process.version });
    recordTool('rustc', { version: version('rustc', ['--version', '--verbose']) });
    recordTool('cargo', { version: version('cargo', ['--version']) });
    await operation();
    outcome = 'success';
  } catch (error) {
    failure = error.message;
    throw error;
  } finally {
    const report = {
      schema: 'conduit.tool-acquisition/v1', target, sourceSha,
      runnerImage: identity.runnerImage, specificationKey: identity.specificationKey,
      outcome, ...(failure ? { failure } : {}),
      durationMs: Math.round(performance.now() - started),
      tools: [...active.tools.values()].sort((a, b) => a.name.localeCompare(b.name)),
      operations: active.operations,
    };
    active = undefined;
    const directory = path.resolve('target/acquisition');
    mkdirSync(directory, { recursive: true });
    writeFileSync(path.join(directory, `${target}.json`), `${JSON.stringify(report, null, 2)}\n`);
    if (outcome === 'success') {
      // Even a lane using only preinstalled tools has an identity to compare in
      // cold/warm experiments. This marker is not a claim of downloaded tools.
      const verified = path.join(homedir(), '.cache/conduit-ci/verified');
      mkdirSync(verified, { recursive: true });
      writeFileSync(path.join(verified, `${target}.json`), JSON.stringify({
        specificationKey: identity.specificationKey, tools: report.tools,
      }));
    }
    console.log(`Acquisition ${target}: ${outcome} in ${report.durationMs} ms; report target/acquisition/${target}.json`);
  }
}
