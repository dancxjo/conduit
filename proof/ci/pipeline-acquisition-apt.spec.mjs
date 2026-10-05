import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readdirSync, rmSync, chmodSync, statSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { assertAptResolution } from '../../tools/ci/pipeline/acquisition/apt-resolution.mjs';
import { aptScope } from '../../tools/ci/pipeline/acquisition/apt-scope.mjs';
import { acquireApt } from '../../tools/ci/pipeline/acquisition/apt.mjs';
const bytes = Buffer.from('fixture authenticated deb');
const sha = createHash('sha256').update(bytes).digest('hex');
function fixture(t) {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-apt-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(path.join(root, 'etc/apt/sources.list.d'), { recursive: true });
  writeFileSync(path.join(root, 'etc/os-release'), 'ID=ubuntu\nVERSION_ID=24.04\n');
  writeFileSync(path.join(root, 'etc/apt/sources.list'), 'deb https://azure.archive.ubuntu.com/ubuntu noble main\n');
  mkdirSync(path.join(root, 'usr/share/keyrings'), { recursive: true });
  writeFileSync(path.join(root, 'usr/share/keyrings/ubuntu-archive-keyring.gpg'), 'fixture signing keys');
  const state = { present: false, calls: [], trusted: true, baseline: '', identities: [] };
  const run = (program, args, config = {}) => {
    state.calls.push([program, args]);
    if (program === 'rm') { rmSync(args.at(-1), { recursive: true, force: true }); return ''; }
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
    if (program === 'dpkg') {
      if (args.includes('--compare-versions')) {
        if (state.downgrade) throw new Error('version comparison refused');
        return '';
      }
      return 'amd64\n';
    }
    if (program === 'tee') { writeFileSync(args[0], config.input); return ''; }
    if (program === 'dpkg-deb') return 'Package: tool\nVersion: 1.2\nArchitecture: amd64\n';
    if (program === 'apt-cache') return `Package: tool\nVersion: 1.2\nArchitecture: amd64\nSHA256: ${state.trusted ? sha : '0'.repeat(64)}\n`;
    if (program === 'apt-get') {
      const source = args.find(arg => arg.startsWith('Dir::Etc::sourcelist=')).split('=').slice(1).join('=');
      assert.doesNotMatch(readFileSync(source, 'utf8'), /microsoft|vendor/);
      if (args.includes('update') && state.requiredFailure) throw new Error('required source unavailable');
      if (args.includes('--simulate')) return state.resolution ?? 'Inst tool (1.2 Ubuntu [amd64])\n';
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

test('unrelated vendor outage is excluded and host repository configuration is preserved', t => {
  const { state, options, root } = fixture(t);
  const vendor = path.join(root, 'etc/apt/sources.list.d/vendor.list');
  const vendorText = 'deb https://packages.microsoft.com/ubuntu/24.04/prod noble main\n';
  writeFileSync(vendor, vendorText);
  const ubuntu = path.join(root, 'etc/apt/sources.list');
  const original = readFileSync(ubuntu, 'utf8');
  acquireApt(['tool'], options);
  assert.equal(readFileSync(vendor, 'utf8'), vendorText);
  assert.equal(readFileSync(ubuntu, 'utf8'), original);
  assert.ok(!state.calls.some(([program]) => program === 'tee'));
  const commands = state.calls.filter(([program]) => ['apt-get', 'apt-cache'].includes(program));
  for (const [, args] of commands) {
    assert.ok(args.includes('Dir::Etc::sourceparts=-'));
    assert.ok(args.includes('Dir::Cache::pkgcache='));
    assert.ok(args.some(arg => arg.startsWith('Dir::State::lists=')));
  }
});
test('required source failure refuses both cold and warm acquisition before installation', t => {
  for (const warm of [false, true]) {
    const { state, options } = fixture(t);
    if (warm) acquireApt(['tool'], options);
    state.present = false; state.calls = []; state.requiredFailure = true;
    assert.throws(() => acquireApt(['tool'], options), /required source unavailable/);
    assert.ok(!state.calls.some(([, args]) => args.includes('install')));
  }
});
test('selected signing keys and runner profile bind cache identity; unrelated sources do not', t => {
  const { state, options, root } = fixture(t);
  const initial = acquireApt(['tool'], options).resolutionKey;
  state.present = false;
  writeFileSync(path.join(root, 'etc/apt/sources.list.d/vendor.list'), 'deb https://vendor.invalid/repo noble main\n');
  assert.equal(acquireApt(['tool'], options).resolutionKey, initial);
  state.present = false;
  writeFileSync(path.join(root, 'usr/share/keyrings/ubuntu-archive-keyring.gpg'), 'rotated signing keys');
  assert.notEqual(acquireApt(['tool'], options).resolutionKey, initial);
  state.present = false;
  writeFileSync(path.join(root, 'etc/os-release'), 'ID=ubuntu\nVERSION_ID="26.04"\nVERSION_CODENAME=resolute\n');
  assert.notEqual(acquireApt(['tool'], options).resolutionKey, initial);
});
test('unsupported profile and vendor-only required package refuse without fallback', t => {
  const { state, options, root } = fixture(t);
  writeFileSync(path.join(root, 'etc/os-release'), 'ID=debian\nVERSION_ID=13\n');
  assert.throws(() => acquireApt(['tool'], options), /Unsupported CI APT profile/);
  assert.ok(!state.calls.some(([program]) => program === 'apt-get'));
  writeFileSync(path.join(root, 'etc/os-release'), 'ID=ubuntu\nVERSION_ID=24.04\n');
  const run = options.run;
  options.privileged = (program, args, config) => {
    if (program === 'apt-get' && args.includes('--download-only')) throw new Error('Unable to locate package vendor-only');
    return run(program, args, config);
  };
  assert.throws(() => acquireApt(['vendor-only'], options), /Unable to locate package/);
});
test('each warm attempt refreshes a distinct index directory with strict authentication', t => {
  const { state, options } = fixture(t);
  acquireApt(['tool'], options); state.present = false;
  acquireApt(['tool'], options);
  const updates = state.calls.filter(([program, args]) => program === 'apt-get' && args.includes('update'));
  assert.equal(updates.length, 2);
  assert.notEqual(updates[0][1].find(arg => arg.startsWith('Dir::State::lists=')), updates[1][1].find(arg => arg.startsWith('Dir::State::lists=')));
  for (const [, args] of updates) for (const control of ['APT::Update::Error-Mode=any', 'APT::Get::AllowUnauthenticated=false', 'Acquire::AllowInsecureRepositories=false', 'Acquire::AllowDowngradeToInsecureRepositories=false', 'Acquire::AllowWeakRepositories=false']) assert.ok(args.includes(control));
});

test('selected scope contains only exact Ubuntu suites, components, architecture and keyring', t => {
  const { root } = fixture(t);
  const scope = aptScope(root, 'amd64');
  assert.equal(scope.policy, 'ubuntu-build-prerequisites@1');
  assert.equal(scope.source, `Types: deb\nURIs: https://archive.ubuntu.com/ubuntu\nSuites: noble noble-updates\nComponents: main restricted universe multiverse\nArchitectures: amd64\nSigned-By: ${root}/usr/share/keyrings/ubuntu-archive-keyring.gpg\n\nTypes: deb\nURIs: https://security.ubuntu.com/ubuntu\nSuites: noble-security\nComponents: main restricted universe multiverse\nArchitectures: amd64\nSigned-By: ${root}/usr/share/keyrings/ubuntu-archive-keyring.gpg\n`);
  assert.throws(() => aptScope(root, 'arm64'), /Unsupported/);
  writeFileSync(path.join(root, 'etc/os-release'), 'ID=ubuntu\nVERSION_ID=24.04\nVERSION_CODENAME=jammy\n');
  assert.throws(() => aptScope(root, 'amd64'), /Unsupported/);
});

test('resolution permits authenticated same-version reinstalls but refuses actual downgrades and removals', () => {
  const installed = new Map([['library:amd64', '2.0'], ['portable:all', '1.0']]);
  const compare = (next, previous) => { if (Number(next) < Number(previous)) throw new Error('older version'); };
  assert.doesNotThrow(() => assertAptResolution('Inst library [2.0] (2.0 Ubuntu [amd64])\nInst new (1.0 Ubuntu [amd64])', installed, 'amd64', compare));
  assert.doesNotThrow(() => assertAptResolution('Inst library:amd64 [2.0] (3.0 Ubuntu [amd64])', installed, 'amd64', compare));
  assert.throws(() => assertAptResolution('Inst library [2.0] (1.0 Ubuntu [amd64])', installed, 'amd64', compare), /would downgrade/);
  assert.throws(() => assertAptResolution('Inst portable [1.0] (0.9 Ubuntu [all])', installed, 'amd64', compare), /would downgrade/);
  assert.throws(() => assertAptResolution('Remv library [2.0]', installed, 'amd64', compare), /would remove/);
  assert.throws(() => assertAptResolution('Inst library unrecognized', installed, 'amd64', compare), /Malformed/);
  assert.throws(() => assertAptResolution('', installed, 'amd64', compare), /no installation/);
});
test('cold and warm resolution reject version decreases before package effects', t => {
  for (const warm of [false, true]) {
    const { state, options } = fixture(t);
    state.baseline = 'library\t2.0\tamd64\tinstalled\n';
    state.resolution = 'Inst library [2.0] (2.0 Ubuntu [amd64])\nInst tool (1.2 Ubuntu [amd64])\n';
    if (warm) {
      acquireApt(['tool'], options);
      state.present = false;
    }
    state.resolution = 'Inst library [2.0] (1.0 Ubuntu [amd64])\nInst tool (1.2 Ubuntu [amd64])\n';
    state.downgrade = true;
    state.calls = [];
    assert.throws(() => acquireApt(['tool'], options), /would downgrade library/);
    assert.ok(!state.calls.some(([, args]) => args.includes('--download-only') || args.includes('--no-download')));
  }
});

test('authentication enumerates archive records even when an installed same-version record has no digest', t => {
  const { state, options } = fixture(t);
  const run = options.run;
  options.run = (program, args, config) => {
    if (program === 'apt-cache' && (!args.includes('--all-versions') || args.at(-1).includes('='))) {
      return 'Package: tool\nVersion: 1.2\nArchitecture: amd64\n';
    }
    return run(program, args, config);
  };
  assert.equal(acquireApt(['tool'], options).cache, 'miss');
  state.present = false;
  assert.equal(acquireApt(['tool'], options).cache, 'hit');
  for (const [program, args] of state.calls) if (program === 'apt-cache') {
    assert.ok(args.includes('--all-versions'));
    assert.equal(args.at(-1), 'tool:amd64');
  }
});
