import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareHostedUnitStorage } from '../../tools/ci/pipeline/runner-storage.mjs';

const hosted = { GITHUB_ACTIONS: 'true', RUNNER_ENVIRONMENT: 'github-hosted' };
const android = '/usr/local/lib/android';
const dotnet = '/usr/share/dotnet';
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
    assert.deepEqual(prepareHostedUnitStorage(options), { skipped: true });
    assert.deepEqual(calls, []);
  }
});

test('hosted cleanup admits only the two fixed image SDK roots and records capacity', () => {
  const { options, calls } = fixture();
  const result = prepareHostedUnitStorage(options);
  assert.deepEqual(calls, [{ program: 'sudo', args: [
    'rm', '--recursive', '--force', '--one-file-system', '--', android, dotnet,
  ] }]);
  assert.deepEqual(result, { skipped: false, beforeBytes: 409600, afterBytes: 819200,
    removed: [android, dotnet], preserved: [] });
});

test('missing SDKs require no privileged operation', () => {
  const { options, calls } = fixture({ inspect: () => {
    throw Object.assign(new Error('absent'), { code: 'ENOENT' });
  } });
  assert.deepEqual(prepareHostedUnitStorage(options).removed, []);
  assert.deepEqual(calls, []);
});

test('files, symlinks and redirected parents are preserved', () => {
  for (const overrides of [
    { inspect: () => ({ isDirectory: () => false, isSymbolicLink: () => false }) },
    { inspect: () => ({ isDirectory: () => true, isSymbolicLink: () => true }) },
    { canonicalPath: () => '/home/runner/work/source' },
  ]) {
    const { options, calls } = fixture(overrides);
    assert.deepEqual(prepareHostedUnitStorage(options).preserved, [android, dotnet]);
    assert.deepEqual(calls, []);
  }
});

test('inspection and cleanup failures cannot become successful capacity proof', () => {
  const failure = new Error('permission denied');
  const inspection = fixture({ inspect: () => { throw failure; } });
  assert.throws(() => prepareHostedUnitStorage(inspection.options), error => error === failure);
  assert.deepEqual(inspection.calls, []);
  const removal = fixture({ run: () => { throw failure; } });
  assert.throws(() => prepareHostedUnitStorage(removal.options), error => error === failure);
});
