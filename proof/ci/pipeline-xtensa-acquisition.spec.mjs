import assert from 'node:assert/strict';
import test from 'node:test';
import { acquireXtensa } from '../../tools/ci/pipeline/targets/setup.mjs';

const install = ['install', '--name', 'esp-conduit-1.91.1', '--targets', 'esp32s3'];
const uninstall = ['uninstall', '--name', 'esp-conduit-1.91.1'];

test('successful Xtensa acquisition installs once', () => {
  const calls = [];
  acquireXtensa('espup', install, {
    run: (program, args) => calls.push([program, args]),
    verify: () => calls.push(['verify']),
  });
  assert.deepEqual(calls, [['espup', install], ['verify']]);
});

test('failed Xtensa acquisition removes its partial toolchain before retry', () => {
  const calls = [];
  acquireXtensa('espup', install, {
    run: (program, args) => {
      calls.push([program, args]);
      if (calls.length === 1) throw new Error('HTTP 503');
    },
    verify: () => calls.push(['verify']),
  });
  assert.deepEqual(calls, [['espup', install], ['espup', uninstall], ['espup', install], ['verify']]);
});

test('a successful but incomplete Xtensa install is cleaned before retry', () => {
  const calls = [];
  acquireXtensa('espup', install, {
    run: (program, args) => calls.push([program, args]),
    verify: () => {
      calls.push(['verify']);
      if (calls.length === 2) throw new Error('GCC unavailable');
    },
  });
  assert.deepEqual(calls, [['espup', install], ['verify'], ['espup', uninstall], ['espup', install], ['verify']]);
});

test('Xtensa acquisition makes only one clean retry', () => {
  const calls = [];
  assert.throws(() => acquireXtensa('espup', install, {
    run: (program, args) => {
      calls.push([program, args]);
      if (args[0] === 'install') throw new Error('HTTP 503');
    },
    verify: () => calls.push(['verify']),
  }), /HTTP 503/);
  assert.deepEqual(calls, [['espup', install], ['espup', uninstall], ['espup', install]]);
});

test('a second incomplete Xtensa install fails rather than emitting a receipt', () => {
  const calls = [];
  assert.throws(() => acquireXtensa('espup', install, {
    run: (program, args) => calls.push([program, args]),
    verify: () => { calls.push(['verify']); throw new Error('GCC unavailable'); },
  }), /GCC unavailable/);
  assert.deepEqual(calls, [['espup', install], ['verify'], ['espup', uninstall], ['espup', install], ['verify']]);
});

test('failed cleanup does not install on top of a partial toolchain', () => {
  const calls = [];
  assert.throws(() => acquireXtensa('espup', install, {
    run: (program, args) => {
      calls.push([program, args]);
      throw new Error(args[0] === 'uninstall' ? 'cleanup failed' : 'HTTP 503');
    },
    verify: () => calls.push(['verify']),
  }), /cleanup failed/);
  assert.deepEqual(calls, [['espup', install], ['espup', uninstall]]);
});
