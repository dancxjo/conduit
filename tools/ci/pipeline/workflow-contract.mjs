// Compile the actual std-only Rust contract without building xtask's dependency
// graph. The actual Rust contract executes alongside independent CLI proof.
import { spawnSync } from 'node:child_process';
import { lstatSync, mkdtempSync, readdirSync, realpathSync, rmSync, statfsSync } from 'node:fs';
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
  const invoke = (program, args, timeout, stage, showOutput = true) => {
    const result = command(program, args, { cwd: directory, encoding: 'utf8', timeout,
      maxBuffer: 8 * 1024 * 1024, env: { ...process.env,
        CARGO_MANIFEST_DIR: path.join(directory, 'tools/xtask') } });
    if (showOutput && result.stdout) emit(result.stdout, 'stdout');
    if (result.stderr) emit(result.stderr, 'stderr');
    if (result.error || result.signal || result.status !== 0) throw new Error(`Workflow contract ${stage} failed: ${result.error?.message ?? result.signal ?? result.status}`);
    return result.stdout ?? '';
  };
  try {
    invoke('rustc', ['--test', '--edition=2021', '-D', 'warnings', '-C', 'debuginfo=0', source, '-o', binary], 120000, 'compilation');
    const executable = lstatSync(binary);
    if (!executable.isFile() || executable.isSymbolicLink() || executable.size < 1 ||
        executable.size > 32 * 1024 * 1024 || readdirSync(output).some(file => file !== name)) throw new Error('Workflow contract executable exceeds private output bound');
    const inventory = invoke(binary, ['--list', '--format=terse'], 10000, 'inventory', false)
      .split(/\r?\n/).filter(line => line.endsWith(': test'));
    if (inventory.length !== 1 || inventory[0] !== 'pipeline_preserves_target_and_artifact_identity: test') throw new Error('Workflow contract must retain its single expected Rust test');
    invoke(binary, ['--nocapture', '--test-threads=1'], 600000, 'execution');
    return { compiled: true, executed: true, executableBytes: executable.size };
  } finally {
    rmSync(output, { recursive: true, force: true });
  }
}
