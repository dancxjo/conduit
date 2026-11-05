import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareHostedStorage } from '../../tools/ci/pipeline/runner-storage.mjs';

const hosted = { GITHUB_ACTIONS: 'true', RUNNER_ENVIRONMENT: 'github-hosted' };
const android = '/usr/local/lib/android';
const dotnet = '/usr/share/dotnet';
const imageSdks = [android, dotnet, '/usr/share/swift', '/usr/local/.ghcup', '/usr/local/share/powershell'];
const directory = { isDirectory: () => true, isSymbolicLink: () => false };

function fixture(overrides = {}) {
  const calls = [];
  let measurements = 0;
  return {
    calls,
    options: {
      environment: hosted,
      platform: 'linux',
      inspect: () => directory,
      canonicalPath: value => value,
      space: () => ({ bavail: ++measurements * 100, bsize: 4096 }),
      run: (program, args) => calls.push({ program, args }),
      log: () => {},
      ...overrides,
    },
  };
}

test('local, self-hosted and non-Linux calls perform no inspection or cleanup', () => {
  for (const [environment, platform] of [
    [{}, 'linux'],
    [{ GITHUB_ACTIONS: 'true', RUNNER_ENVIRONMENT: 'self-hosted' }, 'linux'],
    [{ RUNNER_ENVIRONMENT: 'github-hosted' }, 'linux'],
    [hosted, 'darwin'],
    [hosted, 'win32'],
  ]) {
    const { options, calls } = fixture({ environment, platform,
      inspect: () => assert.fail('must not inspect a user machine'),
      space: () => assert.fail('must not inspect a user machine'),
    });
    assert.deepEqual(prepareHostedStorage(options), { skipped: true });
    assert.deepEqual(calls, []);
  }
});

test('hosted cleanup admits only the fixed unused image SDK roots and records capacity', () => {
  const { options, calls } = fixture();
  const result = prepareHostedStorage(options);
  assert.deepEqual(calls, [{ program: 'sudo', args: [
    'rm', '--recursive', '--force', '--one-file-system', '--', ...imageSdks,
  ] }]);
  assert.deepEqual(result, { skipped: false, beforeBytes: 409600, afterBytes: 819200,
    removed: imageSdks, preserved: [] });
});

test('missing SDKs require no privileged operation', () => {
  const { options, calls } = fixture({ inspect: () => {
    throw Object.assign(new Error('absent'), { code: 'ENOENT' });
  } });
  assert.deepEqual(prepareHostedStorage(options).removed, []);
  assert.deepEqual(calls, []);
});

test('files, symlinks and redirected parents are preserved', () => {
  for (const overrides of [
    { inspect: () => ({ isDirectory: () => false, isSymbolicLink: () => false }) },
    { inspect: () => ({ isDirectory: () => true, isSymbolicLink: () => true }) },
    { canonicalPath: () => '/home/runner/work/source' },
  ]) {
    const { options, calls } = fixture(overrides);
    assert.deepEqual(prepareHostedStorage(options).preserved, imageSdks);
    assert.deepEqual(calls, []);
  }
});

test('inspection and cleanup failures cannot become successful capacity proof', () => {
  const failure = new Error('permission denied');
  const inspection = fixture({ inspect: () => { throw failure; } });
  assert.throws(() => prepareHostedStorage(inspection.options), error => error === failure);
  assert.deepEqual(inspection.calls, []);
  const removal = fixture({ run: () => { throw failure; } });
  assert.throws(() => prepareHostedStorage(removal.options), error => error === failure);
});

test('one redirected SDK is preserved without broadening the other cleanup roots', () => {
  for (const preserved of imageSdks) {
    const {options,calls}=fixture({canonicalPath: value => value === preserved ? '/home/runner/work/source' : value});
    const result=prepareHostedStorage(options);
    assert.deepEqual(result.preserved,[preserved]);
    assert.deepEqual(result.removed,imageSdks.filter(p => p !== preserved));
    assert.deepEqual(calls,[{program:'sudo',args:['rm','--recursive','--force','--one-file-system','--',...result.removed]}]);
  }
});
