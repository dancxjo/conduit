import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readdirSync, rmSync, chmodSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { acquireApt, ubuntuMirror } from '../../tools/ci/pipeline/acquisition/apt.mjs';
const bytes = Buffer.from('fixture authenticated deb');
const sha = createHash('sha256').update(bytes).digest('hex');
function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-apt-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(path.join(root, 'etc/apt/sources.list.d'), { recursive: true });
  writeFileSync(path.join(root, 'etc/os-release'), 'ID=ubuntu\nVERSION_ID=24.04\n');
  writeFileSync(path.join(root, 'etc/apt/sources.list'), 'deb https://azure.archive.ubuntu.com/ubuntu noble main\n');
  const state = { present: false, calls: [], trusted: true, baseline: '', identities: [] };
  const run = (program, args, config = {}) => {
    state.calls.push([program, args]);
    if (program === 'chmod') {
      const walk = directory => {
        chmodSync(directory, statSync(directory).mode | 0o555);
        for (const entry of readdirSync(directory, { withFileTypes: true })) {
          const file = path.join(directory, entry.name);
          if (entry.isDirectory()) walk(file);
          else chmodSync(file, statSync(file).mode | 0o444);
        }
      };
      walk(args[2]); return '';
    }
    if (program === 'dpkg-query') return state.baseline + (state.present ? 'tool\t1.2\tamd64\tinstalled\n' : '');
    if (program === 'dpkg') return 'amd64\n';
    if (program === 'tee') { writeFileSync(args[0], config.input); return ''; }
    if (program === 'dpkg-deb') return 'Package: tool\nVersion: 1.2\nArchitecture: amd64\n';
    if (program === 'apt-cache') return `Package: tool\nVersion: 1.2\nArchitecture: amd64\nSHA256: ${state.trusted ? sha : '0'.repeat(64)}\n`;
    if (program === 'apt-get') {
      if (args.includes('--download-only')) {
        const archives = args.find(value => value.startsWith('Dir::Cache::archives=')).split('=').slice(1).join('=');
        writeFileSync(path.join(archives, 'tool_1.2_amd64.deb'), bytes);
        mkdirSync(path.join(archives, 'partial'), { mode: 0o700, recursive: true });
        writeFileSync(path.join(archives, 'lock'), '', { mode: 0o640 });
      }
      if (args.includes('--no-download')) state.present = true;
      return '';
    }
    throw new Error(`Unexpected command ${program}`);
  };
  const options = { root, cacheRoot: path.join(root, 'cache'), run, privileged: run, imageVersion: 'test-image', report() {}, recordTool(name, identity) { state.identities.push({ name, ...identity }); } };
  return { state, options, root };
}
test('only official Azure Ubuntu mirror is normalized', () => {
  assert.equal(ubuntuMirror('https://azure.archive.ubuntu.com/ubuntu https://azure.archive.ubuntu.com.evil/repo https://other/repo'), 'https://archive.ubuntu.com/ubuntu https://azure.archive.ubuntu.com.evil/repo https://other/repo');
});
test('already installed merged packages skip every privileged command', t => {
  const { state, options } = fixture(t); state.present = true;
  const report = acquireApt(['tool', 'tool'], options);
  assert.equal(report.cache, 'installed');
  assert.deepEqual(report.requested, ['tool']);
  assert.ok(state.calls.every(([program]) => ['dpkg-query', 'dpkg'].includes(program)));
});
test('cold downloads then warm fresh baseline verifies and installs exact version without downloads', t => {
  const { state, options } = fixture(t);
  assert.equal(acquireApt(['tool'], options).downloadedBytes, bytes.length);
  state.present = false; state.calls = [];
  const report = acquireApt(['tool'], options);
  assert.equal(report.cache, 'hit');
  assert.equal(report.downloadedBytes, 0);
  assert.equal(state.calls.filter(([program, args]) => program === 'apt-get' && args.includes('update')).length, 1);
  assert.ok(!state.calls.some(([, args]) => args.includes('--download-only')));
  assert.ok(state.calls.some(([, args]) => args.includes('--no-download') && args.includes('tool:amd64=1.2')));
});
test('corrupt cache refuses before install', t => {
  const { state, options } = fixture(t); acquireApt(['tool'], options);
  const key = readdirSync(options.cacheRoot)[0];
  writeFileSync(path.join(options.cacheRoot, key, 'archives/tool_1.2_amd64.deb'), 'corrupt');
  state.present = false; state.calls = [];
  assert.throws(() => acquireApt(['tool'], options), /digest mismatch/);
  assert.ok(!state.calls.some(([, args]) => args.includes('--no-download')));
});
test('cached version absent from authenticated metadata fails closed', t => {
  const { state, options } = fixture(t); acquireApt(['tool'], options);
  state.present = false; state.trusted = false;
  assert.throws(() => acquireApt(['tool'], options), /not authenticated/);
});
test('changed image baseline cannot silently reuse previous resolution', t => {
  const { state, options } = fixture(t); acquireApt(['tool'], options);
  state.present = false;
  assert.equal(acquireApt(['tool'], { ...options, imageVersion: 'new-image' }).cache, 'miss');
  assert.equal(readdirSync(options.cacheRoot).length, 2);
});

test('mixed requests acquire only missing packages and retain installed baseline versions', t => {
  const { state, options } = fixture(t);
  state.baseline = 'existing\t0.9\tamd64\tinstalled\n';
  const report = acquireApt(['existing', 'tool'], options);
  assert.deepEqual(report.installedVersions, { existing: '0.9', tool: '1.2' });
  for (const [program, args] of state.calls) if (program === 'apt-get' && args.includes('install')) {
    assert.ok(args.includes('tool'));
    assert.ok(!args.includes('existing'));
  }
});
test('cold and warm fresh-machine runs retain identical tool identity', t => {
  const { state, options } = fixture(t);
  assert.equal(acquireApt(['tool'], options).cache, 'miss');
  state.present = false;
  assert.equal(acquireApt(['tool'], options).cache, 'hit');
  assert.deepEqual(state.identities[0], state.identities[1]);
  assert.ok(!Object.hasOwn(state.identities[0], 'cache'));
});

test('successful acquisition makes apt private working entries readable for runner cache archival', t => {
  const { state, options } = fixture(t);
  acquireApt(['tool'], options);
  const archive = path.join(options.cacheRoot, readdirSync(options.cacheRoot)[0], 'archives');
  assert.equal(statSync(path.join(archive, 'partial')).mode & 0o005, 0o005);
  assert.equal(statSync(path.join(archive, 'lock')).mode & 0o004, 0o004);
  const install = state.calls.findIndex(([program, args]) => program === 'apt-get' && args.includes('--no-download'));
  const readable = state.calls.findIndex(([program]) => program === 'chmod');
  assert.ok(readable > install);
  assert.deepEqual(state.calls[readable], ['chmod', ['-R', 'a+rX', archive]]);
});

test('foreign architecture does not satisfy an unqualified native request', t => {
  const { state, options } = fixture(t);
  state.baseline = 'tool\t0.8\ti386\tinstalled\n';
  assert.equal(acquireApt(['tool'], options).cache, 'miss');
  assert.ok(state.calls.some(([, args]) => args.includes('--download-only')));
});
test('explicit native architecture does not use a foreign installed package', t => {
  const { state, options } = fixture(t);
  state.baseline = 'tool\t0.8\ti386\tinstalled\n';
  assert.equal(acquireApt(['tool:amd64'], options).installedVersions['tool:amd64'], '1.2');
});
test('explicit installed foreign architecture is satisfied exactly', t => {
  const { state, options } = fixture(t);
  state.baseline = 'tool\t0.8\ti386\tinstalled\n';
  const report = acquireApt(['tool:i386'], options);
  assert.equal(report.cache, 'installed');
  assert.equal(report.installedVersions['tool:i386'], '0.8');
});
test('architecture-independent installed package satisfies a native request', t => {
  const { state, options } = fixture(t);
  state.baseline = 'tool\t0.7\tall\tinstalled\n';
  const report = acquireApt(['tool'], options);
  assert.equal(report.cache, 'installed');
  assert.equal(report.installedVersions.tool, '0.7');
});
