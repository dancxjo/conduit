import assert from 'node:assert/strict';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { acquireXtensa, cleanXtensa } from '../../tools/ci/pipeline/targets/setup.mjs';

const install = ['install', '--name', 'esp-conduit-1.91.1', '--targets', 'esp32s3'];
const clean = ['clean'];

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
    clean: () => calls.push(clean),
  });
  assert.deepEqual(calls, [['espup', install], clean, ['espup', install], ['verify']]);
});

test('a successful but incomplete Xtensa install is cleaned before retry', () => {
  const calls = [];
  acquireXtensa('espup', install, {
    run: (program, args) => calls.push([program, args]),
    verify: () => {
      calls.push(['verify']);
      if (calls.length === 2) throw new Error('GCC unavailable');
    },
    clean: () => calls.push(clean),
  });
  assert.deepEqual(calls, [['espup', install], ['verify'], clean, ['espup', install], ['verify']]);
});

test('Xtensa acquisition makes only one clean retry', () => {
  const calls = [];
  assert.throws(() => acquireXtensa('espup', install, {
    run: (program, args) => {
      calls.push([program, args]);
      if (args[0] === 'install') throw new Error('HTTP 503');
    },
    verify: () => calls.push(['verify']),
    clean: () => calls.push(clean),
  }), /HTTP 503/);
  assert.deepEqual(calls, [['espup', install], clean, ['espup', install]]);
});

test('a second incomplete Xtensa install fails rather than emitting a receipt', () => {
  const calls = [];
  assert.throws(() => acquireXtensa('espup', install, {
    run: (program, args) => calls.push([program, args]),
    verify: () => { calls.push(['verify']); throw new Error('GCC unavailable'); },
    clean: () => calls.push(clean),
  }), /GCC unavailable/);
  assert.deepEqual(calls, [['espup', install], ['verify'], clean, ['espup', install], ['verify']]);
});

test('failed cleanup does not install on top of a partial toolchain', () => {
  const calls = [];
  assert.throws(() => acquireXtensa('espup', install, {
    run: (program, args) => {
      calls.push([program, args]);
      throw new Error('HTTP 503');
    },
    verify: () => calls.push(['verify']),
    clean: () => { calls.push(clean); throw new Error('cleanup failed'); },
  }), /cleanup failed/);
  assert.deepEqual(calls, [['espup', install], clean]);
});

test('cleanup removes only the named Rustup toolchain and its own clang link', t => {
  const directory = mkdtempSync(path.join(tmpdir(), 'conduit-xtensa-clean-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const rustupHome = path.join(directory, 'rustup');
  const root = path.join(rustupHome, 'toolchains', 'esp-conduit-1.91.1');
  const home = path.join(directory, 'home');
  const link = path.join(home, '.espup', 'esp-clang');
  mkdirSync(root, { recursive: true });
  mkdirSync(path.dirname(link), { recursive: true });
  symlinkSync(path.join(root, 'clang', 'lib'), link);
  let calls = 0;
  cleanXtensa({ rustupHome, home, run: (program, args, options) => {
    calls++;
    assert.equal(program, 'rustup');
    assert.deepEqual(args, ['toolchain', 'uninstall', 'esp-conduit-1.91.1']);
    assert.equal(options.env.RUSTUP_HOME, rustupHome);
    rmSync(root, { recursive: true, force: true });
  } });
  assert.equal(calls, 1);
  assert.equal(existsSync(root), false);
  assert.equal(lstatSync(link, { throwIfNoEntry: false }), undefined);
  assert.equal(existsSync(path.dirname(link)), true);
});

test('cleanup refuses a foreign clang link without changing it', t => {
  const directory = mkdtempSync(path.join(tmpdir(), 'conduit-xtensa-foreign-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const rustupHome = path.join(directory, 'rustup');
  const home = path.join(directory, 'home');
  const link = path.join(home, '.espup', 'esp-clang');
  mkdirSync(path.dirname(link), { recursive: true });
  symlinkSync(path.join(directory, 'someone-else', 'clang'), link);
  assert.throws(() => cleanXtensa({ rustupHome, home, run: () => {
    throw new Error('must not run');
  } }), /foreign Xtensa clang link/);
  assert.equal(lstatSync(link).isSymbolicLink(), true);
});
