import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { acquisitionIdentity } from '../../tools/ci/pipeline/acquisition/metrics.mjs';

test('acquisition cache separates source-independent tools by platform, runner image and exact specification', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-acquisition-key-'));
  try {
    mkdirSync(path.join(root, 'tools/ci/pipeline/acquisition'), { recursive: true });
    const file = path.join(root, 'tools/ci/pipeline/acquisition/tools.mjs');
    writeFileSync(file, 'pinned tool v1');
    const options = { root, home: '/test/home', platform: 'linux', arch: 'x64', env: { ImageOS: 'ubuntu24', ImageVersion: '20261001', RUSTUP_TOOLCHAIN: '1.98.1' } };
    const first = acquisitionIdentity('browser', options);
    assert.equal(first.key, acquisitionIdentity('browser', options).key);
    assert.notEqual(first.key, acquisitionIdentity('browser', { ...options, arch: 'arm64' }).key);
    assert.notEqual(first.key, acquisitionIdentity('browser', { ...options, env: { ...options.env, ImageVersion: '20261002' } }).key);
    assert.notEqual(first.key, acquisitionIdentity('browser', { ...options, env: { ...options.env, RUSTUP_TOOLCHAIN: '1.99.0' } }).key);
    writeFileSync(file, 'pinned tool v2');
    assert.notEqual(first.key, acquisitionIdentity('browser', options).key);
  } finally { rmSync(root, { recursive: true }); }
});

test('curated caches exclude product evidence, installed system packages and arbitrary toolchains', () => {
  const options = { root: '/test/repository', home: '/test/home', env: {}, platform: 'linux', arch: 'x64' };
  for (const target of ['preflight', 'unit', 'conduitos-ia32', 'browser', 'avr', 'esp32-c3', 'esp32-s3', 'rp2040']) {
    const { paths } = acquisitionIdentity(target, options);
    assert.ok(paths.every(value => !value.includes('target/pipeline') && !value.includes('/usr') && !value.includes('/var/lib/dpkg')));
    assert.ok(!paths.includes('/test/home/.rustup/toolchains'));
  }
  assert.ok(acquisitionIdentity('browser', options).paths.includes('/test/home/.cache/ms-playwright'));
  assert.ok(!acquisitionIdentity('rp2040', options).paths.includes('/test/home/.cache/ms-playwright'));
  for (const target of ['unit', 'browser', 'hosted-linux', 'conduitos-x86_64',
    'conduitos-aarch64', 'conduitos-ia32', 'conduitos-riscv64',
    'conduitos-loongarch64', 'avr', 'esp32-c3', 'esp32-s3', 'esp32-wroom',
    'raspberry-pi', 'orange-pi', 'rp2040']) {
    assert.equal(acquisitionIdentity(target, options).cacheable, true, target);
  }
  for (const target of ['preflight', 'hosted-windows', 'hosted-macos']) {
    assert.equal(acquisitionIdentity(target, options).cacheable, false, target);
  }
  assert.throws(() => acquisitionIdentity('../../escape', options));
});
