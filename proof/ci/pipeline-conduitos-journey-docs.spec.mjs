import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { deflateSync } from 'node:zlib';

const names = [
  'front-door-ready', 'body-born', 'body-woken', 'body-planned', 'body-playing',
  'protected-keyboard-canvas', 'protected-memory-lantern',
  'home', 'patchbay-workspace', 'patchbay-face', 'patchbay-diagram', 'body-stopped',
];
const commit = '1'.repeat(40);
const image = 'a'.repeat(64);
const boot = 'b'.repeat(64);
const body = 'c'.repeat(64);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

test('Pages composition publishes only the exact twelve-step QMP journey', () => {
  const fixture = makeFixture();
  try {
    const result = publish(fixture);
    assert.equal(result.status, 0, result.stderr);
    const root = path.join(fixture.site, 'journeys/current/conduitos/x86_64');
    const page = readFileSync(path.join(root, 'index.html'), 'utf8');
    for (const name of names) assert.match(page, new RegExp(`src="${name}\\.png"`));
    assert.match(page, /A Body, its Patchbay, and its Face/);
    assert.match(page, /What changed:/);
    assert.match(page, /twelve screenshots/);
    assert.equal((page.match(/class="journey-step"/g) ?? []).length, 12);
    const retained = readFileSync(path.join(fixture.site, `journeys/commits/${commit}/conduitos/x86_64/index.html`), 'utf8');
    for (const name of names) assert.ok(retained.includes(`src="${name}.png"`));
    assert.match(page, /Main navigation/);
    assert.match(page, /cargo xtask make conduitos journey-proof/);
    assert.match(page, /Journey serial transcript/);
    assert.doesNotMatch(page, /Tour chapter/);
    const site = readFileSync(path.join(fixture.site, 'site-publication.json'), 'utf8');
    assert.equal(JSON.parse(site).conduitos.sourceCommit, commit);
    assert.equal(JSON.parse(site).conduitos.serialSha256,
      digest(readFileSync(path.join(root, 'console.txt'))));
    assert.match(readFileSync(path.join(fixture.site, 'journeys/index.html'), 'utf8'),
      /Follow the ConduitOS journey/);
    assert.deepEqual(readFileSync(path.join(root, 'patchbay-diagram.png')),
      readFileSync(path.join(fixture.evidence, 'patchbay-diagram.png')));
    assert.deepEqual(readFileSync(path.join(root, 'console.txt')),
      readFileSync(path.join(fixture.evidence, '../journey-serial.log')));
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('Pages composition refuses a changed screenshot before writing pages', () => {
  const fixture = makeFixture();
  try {
    writeFileSync(path.join(fixture.evidence, 'patchbay-diagram.png'), png(99));
    const result = publish(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /differs from its capture/);
    assert.equal(readFileSync(path.join(fixture.site, 'journeys/index.html'), 'utf8'),
      '<!-- conduit-conduitos-journey-card@1 -->');
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test('Pages composition refuses a transcript from another Body before writing pages', () => {
  const fixture = makeFixture();
  try {
    writeFileSync(path.join(fixture.evidence, '../journey-serial.log'),
      `CONDUIT_PRODUCT_JOURNEY {"boot_id":"${boot}","body_id":"other"}\n`);
    const result = publish(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /serial transcript does not match/);
    assert.equal(readFileSync(path.join(fixture.site, 'journeys/index.html'), 'utf8'),
      '<!-- conduit-conduitos-journey-card@1 -->');
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

for (const checkpoint of ['protected-keyboard-canvas', 'protected-memory-lantern']) {
  test(`Pages composition refuses a journey missing ${checkpoint}`, () => {
    const fixture = makeFixture();
    try {
      const manifestPath = path.join(fixture.evidence, 'manifest.json');
      const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
      manifest.checkpoints = manifest.checkpoints.filter(entry => entry.checkpoint !== checkpoint);
      writeFileSync(manifestPath, JSON.stringify(manifest));
      const result = publish(fixture);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /journey is incomplete/);
    } finally {
      rmSync(fixture.root, { recursive: true, force: true });
    }
  });
}

test('Pages composition refuses reordered captures', () => {
  const fixture = makeFixture();
  try {
    const manifestPath = path.join(fixture.evidence, 'manifest.json');
    const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
    [manifest.checkpoints[5], manifest.checkpoints[6]] = [manifest.checkpoints[6], manifest.checkpoints[5]];
    writeFileSync(manifestPath, JSON.stringify(manifest));
    const result = publish(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /checkpoint 6 violates/);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

function makeFixture() {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-journey-docs-'));
  const evidence = path.join(root, 'evidence/journey-frames');
  const site = path.join(root, 'site');
  mkdirSync(evidence, { recursive: true });
  mkdirSync(path.join(site, 'journeys'), { recursive: true });
  writeFileSync(path.join(site, 'journeys/index.html'), '<!-- conduit-conduitos-journey-card@1 -->');
  writeFileSync(path.join(site, 'site-publication.json'), JSON.stringify({ sourceCommit: commit }));
  const checkpoints = names.map((checkpoint, index) => {
    const bytes = png(index);
    writeFileSync(path.join(evidence, `${checkpoint}.png`), bytes);
    return {
      checkpoint, unchanged: false, expected_change: index !== 0, health_refusal: null,
      guest_boot_record: { boot_id: boot },
      frame: {
        checkpoint, width: 1280, height: 800, pixel_format: 'RGBA8',
        non_background_pixels: 1000, png: `${checkpoint}.png`,
        png_bytes: bytes.length, png_sha256: digest(bytes),
      },
    };
  });
  writeFileSync(path.join(evidence, 'manifest.json'), JSON.stringify({
    schema: 'conduit.conduitos/visual-journey@1', proof_class: 'freestanding-emulator',
    status: 'complete', failure: null, context: { source_commit: commit, image_sha256: image },
    checkpoints,
  }));
  writeFileSync(path.join(root, 'evidence/journey-proof.json'), JSON.stringify({
    schema: 'conduit.conduitos/face-journey-proof@1', source_commit: commit,
    proof_class: 'freestanding-emulator', image_sha256: image,
    screenshots: 'journey-frames/manifest.json', boot_id: boot, body_id: body,
  }));
  writeFileSync(path.join(root, 'evidence/journey-serial.log'),
    `CONDUIT_PRODUCT_JOURNEY {"boot_id":"${boot}","body_id":"${body}"}\n`);
  return { root, evidence, site };
}

function publish({ evidence, site }) {
  return spawnSync(process.execPath,
    ['tools/ci/stage-conduitos-pages-evidence.mjs', evidence, site, commit],
    { encoding: 'utf8' });
}

function png(marker) {
  const stride = 1 + 1280 * 4;
  const raw = Buffer.alloc(stride * 800);
  raw[1] = marker;
  raw[4] = 255;
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(1280, 0);
  ihdr.writeUInt32BE(800, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function chunk(name, data) {
  const type = Buffer.from(name);
  const bytes = Buffer.concat([type, data]);
  const output = Buffer.alloc(bytes.length + 8);
  output.writeUInt32BE(data.length, 0);
  bytes.copy(output, 4);
  output.writeUInt32BE(crc(bytes), output.length - 4);
  return output;
}

function crc(bytes) {
  let value = 0xffffffff;
  for (const byte of bytes) {
    value ^= byte;
    for (let i = 0; i < 8; i++) value = value >>> 1 ^ (value & 1 ? 0xedb88320 : 0);
  }
  return (value ^ 0xffffffff) >>> 0;
}
