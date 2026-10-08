import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');
const commit = 'a'.repeat(40);
const checkpoints = [
  'front-door-ready', 'body-born', 'body-woken', 'body-planned', 'body-playing',
  'protected-keyboard-canvas', 'protected-memory-lantern', 'home',
  'patchbay-workspace', 'patchbay-face', 'patchbay-diagram', 'body-stopped',
];

async function fixture(t) {
  const directory = await mkdtemp(path.join(tmpdir(), 'conduit-pages-spec-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const evidence = path.join(directory, 'evidence');
  const frames = path.join(evidence, 'journey-frames');
  const site = path.join(directory, 'site');
  await mkdir(frames, { recursive: true });
  await mkdir(path.join(site, 'journeys'), { recursive: true });
  // Synthetic header exercises staging's existing capture checks. These bytes
  // are a test fixture, never a product screenshot or emulator proof.
  const png = Buffer.alloc(24);
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(png);
  png.write('IHDR', 12);
  png.writeUInt32BE(1280, 16);
  png.writeUInt32BE(800, 20);
  const manifest = {
    schema: 'conduit.conduitos/visual-journey@1', proof_class: 'freestanding-emulator',
    status: 'complete', failure: null,
    context: { source_commit: commit, image_sha256: 'image-fixture' },
    checkpoints: checkpoints.map((checkpoint, index) => ({
      checkpoint, health_refusal: null, unchanged: false, expected_change: index !== 0,
      guest_boot_record: { boot_id: 'boot-fixture' },
      frame: {
        checkpoint, width: 1280, height: 800, pixel_format: 'RGBA8', non_background_pixels: 100,
        png: `${checkpoint}.png`, png_bytes: png.length,
        png_sha256: createHash('sha256').update(png).digest('hex'),
      },
    })),
  };
  for (const checkpoint of checkpoints) await writeFile(path.join(frames, `${checkpoint}.png`), png);
  await writeFile(path.join(evidence, 'journey-proof.json'), JSON.stringify({
    schema: 'conduit.conduitos/face-journey-proof@1', source_commit: commit,
    proof_class: 'freestanding-emulator', image_sha256: 'image-fixture',
    screenshots: 'journey-frames/manifest.json', boot_id: 'boot-fixture', body_id: 'body-fixture',
  }));
  await writeFile(path.join(evidence, 'journey-serial.log'),
    `CONDUIT_PRODUCT_JOURNEY boot-fixture body-fixture ${'fixture '.repeat(8)}`);
  await writeFile(path.join(site, 'journeys/index.html'), '<!-- conduit-conduitos-journey-card@1 -->');
  await writeFile(path.join(site, 'site-publication.json'), JSON.stringify({ sourceCommit: commit }));
  const run = async () => {
    await writeFile(path.join(frames, 'manifest.json'), JSON.stringify(manifest));
    return spawnSync(process.execPath, ['tools/ci/stage-conduitos-pages-evidence.mjs', frames, site, commit],
      { cwd: root, encoding: 'utf8' });
  };
  return { manifest, frames, site, run };
}

test('Pages stages all twelve captured journey checkpoints in current and retained pages', async t => {
  const { site, run } = await fixture(t);
  const result = await run();
  assert.equal(result.status, 0, result.stderr);
  for (const prefix of ['current', `commits/${commit}`]) {
    const html = await readFile(path.join(site, 'journeys', prefix, 'conduitos/x86_64/index.html'), 'utf8');
    assert.equal((html.match(/class="journey-step"/g) ?? []).length, 12);
    for (const checkpoint of checkpoints) assert.ok(html.includes(`id="${checkpoint}"`));
    assert.ok(html.includes('twelve screenshots'));
  }
});

for (const checkpoint of ['protected-keyboard-canvas', 'protected-memory-lantern']) {
  test(`Pages refuses a journey missing ${checkpoint}`, async t => {
    const { manifest, run } = await fixture(t);
    manifest.checkpoints = manifest.checkpoints.filter(entry => entry.checkpoint !== checkpoint);
    const result = await run();
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /journey is incomplete/);
  });
}

test('Pages refuses reordered checkpoints and screenshots altered after capture', async t => {
  const { manifest, frames, run } = await fixture(t);
  [manifest.checkpoints[5], manifest.checkpoints[6]] = [manifest.checkpoints[6], manifest.checkpoints[5]];
  let result = await run();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /checkpoint 6 violates/);
  [manifest.checkpoints[5], manifest.checkpoints[6]] = [manifest.checkpoints[6], manifest.checkpoints[5]];
  await writeFile(path.join(frames, 'protected-memory-lantern.png'), 'altered-fixture');
  result = await run();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /screenshot protected-memory-lantern differs/);
});
