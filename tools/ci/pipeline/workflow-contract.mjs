// Compile the actual std-only Rust contract without building xtask's dependency
// graph. The actual Rust contract executes alongside independent CLI proof.
import { spawnSync } from 'node:child_process';
import { lstatSync, mkdtempSync, readdirSync, realpathSync, rmSync, statfsSync, mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

export function runWorkflowContract({ root = process.cwd(), temporaryParent = tmpdir(),
  command = spawnSync, emit = (text, stream) => process[stream].write(text) } = {}) {
  if (!path.isAbsolute(root) || !path.isAbsolute(temporaryParent)) throw new Error('Absolute workflow contract directories required');
  const directory = realpathSync(root);
  const source = path.join(directory, 'tools/xtask/tests/ci_workflow_contract.rs');
  const info = lstatSync(source);
  if (!info.isFile() || info.isSymbolicLink() || info.size > 1024 * 1024) throw new Error('Unsafe workflow contract source');
  const parent = realpathSync(temporaryParent);
  const disk = statfsSync(parent);
  if (disk.bavail * disk.bsize < 128 * 1024 * 1024) throw new Error('Workflow contract temporary reserve unavailable');
  const output = mkdtempSync(path.join(parent, 'conduit-workflow-contract-'));
  const name = process.platform === 'win32' ? 'contract.exe' : 'contract';
  const binary = path.join(output, name);
  const invoke = (program, args, timeout, stage, showOutput = true, extraEnvironment = {}, allowFailure = false) => {
    const environment = { ...process.env, CARGO_MANIFEST_DIR: path.join(directory, 'tools/xtask') };
    if (process.platform === 'win32' && Object.hasOwn(extraEnvironment, 'PATH')) {
      for (const key of Object.keys(environment)) if (key.toLowerCase() === 'path') delete environment[key];
    }
    Object.assign(environment, extraEnvironment);
    const result = command(program, args, { cwd: directory, encoding: 'utf8', timeout,
      maxBuffer: 8 * 1024 * 1024, env: environment });
    if (showOutput && result.stdout) emit(result.stdout, 'stdout');
    if (showOutput && result.stderr) emit(result.stderr, 'stderr');
    if (result.error || result.signal || (!allowFailure && result.status !== 0)) throw new Error(`Workflow contract ${stage} failed: ${result.error?.message ?? result.signal ?? result.status}`);
    return allowFailure ? result : (result.stdout ?? '');
  };
  try {
    invoke('rustc', ['--test', '--edition=2021', '-D', 'warnings', '-C', 'debuginfo=0', source, '-o', binary], 120000, 'compilation');
    const executable = lstatSync(binary);
    if (!executable.isFile() || executable.isSymbolicLink() || executable.size < 1 ||
        executable.size > 32 * 1024 * 1024 || readdirSync(output).some(file => file !== name)) throw new Error('Workflow contract executable exceeds private output bound');
    const inventory = invoke(binary, ['--list', '--format=terse'], 10000, 'inventory', false)
      .split(/\r?\n/).filter(line => line.endsWith(': test'));
    if (inventory.length !== 1 || inventory[0] !== 'pipeline_preserves_target_and_artifact_identity: test') throw new Error('Workflow contract must retain its single expected Rust test');
    const files = readdirSync(path.join(directory, 'proof/ci')).filter(name =>
      (name.startsWith('pipeline-') || name.startsWith('compiler-cache')) && name.endsWith('.spec.mjs'))
      .map(name => path.join(directory, 'proof/ci', name)).sort();
    if (!files.length || files.length > 4096) throw new Error('Workflow contract canonical Node inventory invalid');
    for (const file of files) {
      const info = lstatSync(file);
      if (!info.isFile() || info.isSymbolicLink()) throw new Error('Workflow contract canonical Node inventory invalid');
    }
    const proxyDirectory = path.join(output, 'proxy'); mkdirSync(proxyDirectory, { mode: 0o700 });
    const proxy = path.join(proxyDirectory, 'launch.mjs');
    const rustString = value => {
      let hashes = '#';
      while (value.includes('"' + hashes)) hashes += '#';
      return `r${hashes}"${value}"${hashes}`;
    };
    const launcherSource = path.join(proxyDirectory, 'node.rs');
    const launcher = path.join(proxyDirectory, process.platform === 'win32' ? 'node.exe' : 'node');
    const launcherBytes = `fn main() {
 let status = std::process::Command::new(${rustString(process.execPath)})
 .arg(${rustString(proxy)}).args(std::env::args_os().skip(1)).status();
 std::process::exit(match status { Ok(value) => value.code().unwrap_or(99), Err(_) => 99 });
}
`;
    if (Buffer.byteLength(launcherBytes) > 65536) throw new Error('Workflow contract Node proxy source exceeds bound');
    writeFileSync(launcherSource, launcherBytes, { mode: 0o600 });
    invoke('rustc', ['--edition=2021', '-D', 'warnings', '-C', 'debuginfo=0', launcherSource, '-o', launcher],
      120000, 'Node proxy compilation');
    const launcherInfo = lstatSync(launcher);
    if (!launcherInfo.isFile() || launcherInfo.isSymbolicLink() || launcherInfo.size < 1
        || launcherInfo.size > 32 * 1024 * 1024
        || readdirSync(proxyDirectory).some(file => !['node.rs', path.basename(launcher)].includes(file))) {
      throw new Error('Workflow contract Node proxy executable exceeds private output bound');
    }
    for (const mode of ['passing', 'failing']) {
      const probe = path.join(output, `${mode}.spec.mjs`);
      const receipt = path.join(output, `${mode}.receipt.json`);
      const calls = path.join(output, `${mode}.calls`);
      writeFileSync(probe, `import test from 'node:test'; import assert from 'node:assert/strict';\ntest('bounded launcher ${mode} probe', () => assert.ok(${mode === 'passing'}));\n`, { mode: 0o600 });
      // This proxy observes the real Rust Command invocation, then runs a real
      // small Node test. The full invariant corpus runs independently in CLI.
      writeFileSync(proxy, `import fs from 'node:fs'; import path from 'node:path'; import {spawnSync} from 'node:child_process';
const expected=${JSON.stringify(files)}, root=${JSON.stringify(directory)};
const calls=${JSON.stringify(calls)}, receipt=${JSON.stringify(receipt)};
try {
 fs.appendFileSync(calls,'1');
 const args=process.argv.slice(2), actual=args.slice(1).map(file=>path.resolve(file)).sort();
 if(fs.realpathSync(process.cwd())!==root || args[0]!=='--test' || args.length!==expected.length+1 || new Set(actual).size!==actual.length || JSON.stringify(actual)!==JSON.stringify(expected)) process.exit(97);
 // A parent node --test child context suppresses nested test execution.
 const probeEnvironment={...process.env}; delete probeEnvironment.NODE_TEST_CONTEXT;
 const result=spawnSync(${JSON.stringify(process.execPath)},['--test',${JSON.stringify(probe)}],{cwd:root,env:probeEnvironment,encoding:'utf8',timeout:10000,maxBuffer:131072});
 if(result.error || result.signal || !Number.isInteger(result.status)) process.exit(98);
 fs.writeFileSync(receipt,JSON.stringify({mode:${JSON.stringify(mode)},verified:true,status:result.status}),{flag:'wx',mode:0o600});
 if(result.stdout) process.stdout.write(result.stdout); if(result.stderr) process.stderr.write(result.stderr);
 process.exit(result.status);
} catch { process.exit(99); }
`, { mode: 0o600 });
      const result = invoke(binary, ['--nocapture', '--test-threads=1'], 30000,
        `${mode} launch probe`, mode === 'passing', { PATH: `${proxyDirectory}${path.delimiter}${Object.entries(process.env).find(([key]) => key.toLowerCase() === 'path')?.[1] ?? ''}` }, true);
      const boundedRead = filename => {
        const info = lstatSync(filename);
        if (!info.isFile() || info.isSymbolicLink() || info.nlink !== 1 || info.size > 4096) throw new Error('Workflow contract launch probe receipt invalid');
        return readFileSync(filename, 'utf8');
      };
      let record;
      try {
        if (boundedRead(calls) !== '1') throw new Error();
        record = JSON.parse(boundedRead(receipt));
      } catch { throw new Error(`Workflow contract ${mode} Node launch unverified`); }
      if (record.mode !== mode || record.verified !== true || !Number.isInteger(record.status)
          || (mode === 'passing' ? record.status !== 0 || result.status !== 0
            : record.status !== 1 || result.status === 0 || !Number.isInteger(result.status))) {
        throw new Error(`Workflow contract ${mode} Node failure propagation invalid`);
      }
      if (mode === 'failing') emit('Workflow contract expected failing Node probe propagated successfully.\n', 'stdout');
    }
    return { compiled: true, executed: true, executableBytes: executable.size,
      canonicalNodeFiles: files.length, passingNodeProbe: true, failingNodeProbe: true };
  } finally {
    rmSync(output, { recursive: true, force: true });
  }
}
