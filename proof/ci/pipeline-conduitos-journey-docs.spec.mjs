import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { deflateSync } from 'node:zlib';
import { sealTarget } from '../../tools/ci/pipeline/receipts.mjs';
import { pointerFixture } from './conduitos-pointer-fixture.mjs';
import { lookFixture } from './conduitos-look-fixture.mjs';

const names = [
  'front-door-ready', 'creche-ready',
  'creche-editorial-compact', 'creche-editorial-spacious', 'creche-engineering-spacious', 'creche-engineering-compact',
  'creche-three-selected', 'creche-four-restored', 'creche-fourth-checked', 'creche-pointer-three-selected', 'creche-pointer-four-restored', 'body-playing', 'wake-refused',
  'protected-keyboard-canvas', 'protected-memory-lantern',
  'home', 'patchbay-workspace', 'patchbay-face', 'patchbay-diagram',
  'patchbay-gear-details', 'patchbay-diagram-returned', 'body-stopped', 'home-returned', 'patchbay-reentered',
];
const commit = '1'.repeat(40);
const image = 'a'.repeat(64);
const boot = 'b'.repeat(64);
const body = 'c'.repeat(64);
const host = 'd'.repeat(64);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

test('Pages composition publishes only the exact refusal and reentry QMP journey', async () => {
  const fixture = makeFixture();
  try {
    const result = publish(fixture);
    assert.equal(result.status, 0, result.stderr);
    const receipt = await sealTarget({ directory: path.join(fixture.root, 'evidence'),
      target: 'conduitos-x86_64', sha: commit, proofClass: 'emulator' });
    const editFile = receipt.files.find(file => file.path === 'memory-lantern-native-edit.json');
    assert.equal(editFile.sha256, digest(readFileSync(path.join(fixture.evidence, '../memory-lantern-native-edit.json'))));
    const pairedFile = receipt.files.find(file => file.path === 'native-shell-paired-linear.json');
    assert.equal(pairedFile.sha256,
      digest(readFileSync(path.join(fixture.evidence, '../native-shell-paired-linear.json'))));
    const root = path.join(fixture.site, 'journeys/current/conduitos/x86_64');
    const page = readFileSync(path.join(root, 'index.html'), 'utf8');
    for (const name of names) assert.match(page, new RegExp(`src="${name}\\.png"`));
    assert.match(page, /A Body, its Patchbay, and its Face/);
    assert.match(page, /What changed:/);
    assert.match(page, new RegExp(`${names.length} screenshots`));
    assert.equal((page.match(/class="journey-step"/g) ?? []).length, names.length);
    const retained = readFileSync(path.join(fixture.site, `journeys/commits/${commit}/conduitos/x86_64/index.html`), 'utf8');
    for (const name of names) assert.ok(retained.includes(`src="${name}.png"`));
    assert.match(page, /Main navigation/);
    assert.match(page, /cargo \+stable xtask make conduitos graphical-profile-proof/);
    assert.match(page, /Journey serial transcript/);
    assert.match(page, /paired linear reconstruction/);
    assert.match(page, /parent #5356 acceptance remains open/);
    assert.deepEqual(readFileSync(path.join(root, 'native-shell-paired-linear.json')),
      readFileSync(path.join(fixture.evidence, '../native-shell-paired-linear.json')));
    assert.equal(JSON.parse(readFileSync(path.join(fixture.site, 'site-publication.json'))).conduitos.pairedSha256,
      digest(readFileSync(path.join(root, 'native-shell-paired-linear.json'))));
    assert.match(page, /Committed text receipt/);
    assert.match(page, /Text editor retains café/);
    assert.match(page, /Tab · A · Backspace · C · A · F · Right Alt\+E/);
    assert.doesNotMatch(page, /A · B · Backspace/);
    assert.equal(JSON.parse(readFileSync(path.join(fixture.site, 'site-publication.json'))).conduitos.committedEditSha256,
      digest(readFileSync(path.join(root, 'memory-lantern-native-edit.json'))));
    assert.deepEqual(JSON.parse(readFileSync(path.join(root, 'memory-lantern-native-edit.json'))).utf8_bytes,
      [99,97,102,195,169]);
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

for (const checkpoint of ['creche-editorial-compact', 'creche-editorial-spacious', 'creche-engineering-spacious', 'creche-engineering-compact', 'creche-three-selected', 'creche-four-restored', 'wake-refused', 'home-returned', 'patchbay-reentered', 'protected-keyboard-canvas', 'protected-memory-lantern',
  'patchbay-gear-details', 'patchbay-diagram-returned']) {
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
  const look = lookFixture({ commit, image, host, boot });
  const checkpoints = names.map((checkpoint, index) => {
    const bytes = png(index);
    writeFileSync(path.join(evidence, `${checkpoint}.png`), bytes);
    return {
      checkpoint, unchanged: false, expected_change: index !== 0 && !['creche-ready', 'wake-refused'].includes(checkpoint), health_refusal: null,
      ...(look.manifest.checkpoints.find(entry => entry.checkpoint === checkpoint) ?? {}),
      guest_boot_record: { boot_id: boot, host_id: host, build_id: 'build', image_binding: 'image', profile_id: 'profile' },
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
  const paired = pairedFixture();
  const lookPairs = look.faces.map(native => ({ native, linear: {
    proof_class: 'deterministic-semantic-reconstruction', face_id: native.face.identity,
    face_revision: native.face.revision, actions: native.face.actions, basis: native.face.basis,
    context: native.face.interaction_context, lines: ['Look fixture'], acknowledged_show: null, input_enactment: false,
  } }));
  paired.paired_faces.unshift(...lookPairs);
  writeFileSync(path.join(root, 'evidence/journey-proof.json'), JSON.stringify({
    schema: 'conduit.conduitos/face-journey-proof@1', source_commit: commit,
    proof_class: 'freestanding-emulator', image_sha256: image,
    screenshots: 'journey-frames/manifest.json', boot_id: boot, body_id: body, host_id: host,
    paired_reconstruction: 'native-shell-paired-linear.json',
    committed_edit_receipt: 'memory-lantern-native-edit.json', pointer_actions: paired.pointer,
    build_id: 'build', image_id: 'image', profile_id: 'profile', look_selection: look.proof.look_selection,
  }));
  const record = { boot_id: boot, body_id: body, host_id: host, plan_id: 'plan/editor',
    active_play_id: 'play/editor', checked_plot_id: 'checked/editor',
    result_sign_id: 'sign/editor', result: 'café' };
  writeFileSync(path.join(root, 'evidence/memory-lantern-native-edit.json'), JSON.stringify({
    schema: 'conduit.text/native-committed-edit@1', proof_class: 'freestanding-emulator',
    physical_evidence: false, human_enactment: false,
    committed_text: 'café', utf8_bytes: [99,97,102,195,169], record,
  }));
  writeFileSync(path.join(root, 'evidence/native-shell-paired-linear.json'), JSON.stringify(paired));
  writeFileSync(path.join(root, 'evidence/journey-serial.log'),
    look.serial.toString('utf8') + `CONDUIT_PRODUCT_JOURNEY ${JSON.stringify(record)}\n`
    + paired.paired_faces.slice(lookPairs.length).map(pair => `CONDUIT_NATIVE_FACE ${JSON.stringify(pair.native)}\n`).join('')
    + `CONDUIT_NATIVE_BIRTH ${JSON.stringify(paired.birth)}\n`
    + paired.actions.map(action => `CONDUIT_NATIVE_ACTION ${JSON.stringify(action)}\n`).join('')
    + pointerFixture().serial);
  writeFileSync(path.join(root, 'evidence/journey-qmp.log'), 'fixture QMP transcript; no emulator claim\n');
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

// Structural publication fixtures only; native semantic admission is proved by Rust/QEMU.
function pairedFixture() {
  const owner = revision => ({ body_id: body, born_sign_id: 'born/a', revision });
  const native = index => ({ schema: 'conduit.native-face/v1',
    face: { identity: `face/${index}`, revision: index, basis: { body_id: index ? body : null },
      actions: [], interaction_context: { identity: 'context/fixture' } },
    show: { show_id: `show/${index}`, lifecycle: 'Available' } });
  const paired_faces = [0, 1, 2, 3].map(index => {
    const record = native(index);
    return { native: record, linear: { proof_class: 'deterministic-semantic-reconstruction',
      face_id: record.face.identity, face_revision: index, actions: record.face.actions,
      basis: record.face.basis, context: record.face.interaction_context, lines: ['fixture'],
      acknowledged_show: null, input_enactment: false } };
  });
  const receipt = (index, before, after, refused = false) => ({ schema: 'conduit.native-action/v1',
    show_id: `show/${index}`, face_id: `face/${index}`, face_revision: index,
    accepted_effects: refused ? 0 : 1, before, after,
    disposition: refused ? { Refused: { reason: 'UnavailableAction' } } : { Accepted: {} } });
  return { schema: 'conduit.native-shell/paired-linear@1', source_commit: commit, image_sha256: image,
    proof_class: 'freestanding-emulator', body_id: body, birth_count: 1,
    birth: receipt(0, { body_id: null, born_sign_id: null }, owner(1)), paired_faces,
    actions: [receipt(1, owner(1), owner(2)), receipt(2, owner(2), owner(2), true), receipt(3, owner(2), owner(3))],
    accepted_action_count: 2, refused_action_count: 1, fresh_action_after_refusal: true,
    screenshots: 'journey-frames/manifest.json', pointer: pointerFixture().receipt, browser: { status: 'unsupported' },
    parent_5356_acceptance: 'open', physical_evidence: false, human_enactment: false };
}

for (const [label, mutate] of [
  ['source pin', paired => { paired.source_commit = '2'.repeat(40); }],
  ['image pin', paired => { paired.image_sha256 = '2'.repeat(64); }],
  ['schema', paired => { paired.schema = 'wrong'; }],
  ['refusal effect', paired => { paired.actions[1].accepted_effects = 1; }],
  ['refusal state', paired => { paired.actions[1].after.revision++; }],
  ['count', paired => { paired.refused_action_count = 99; }],
  ['linear action contract', paired => { paired.paired_faces[0].linear.actions = ['forged']; }],
]) {
  test(`Pages composition refuses paired evidence with changed ${label}`, () => {
    const fixture = makeFixture();
    try {
      const filename = path.join(fixture.evidence, '../native-shell-paired-linear.json');
      const paired = JSON.parse(readFileSync(filename, 'utf8'));
      mutate(paired);
      writeFileSync(filename, JSON.stringify(paired));
      const result = publish(fixture);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /paired evidence differs/);
      assert.equal(readFileSync(path.join(fixture.site, 'journeys/index.html'), 'utf8'),
        '<!-- conduit-conduitos-journey-card@1 -->');
    } finally { rmSync(fixture.root, { recursive: true, force: true }); }
  });
}

test('Pages composition rejects a refusal screenshot claiming mandatory pixel change', () => {
  const fixture = makeFixture();
  try {
    const filename = path.join(fixture.evidence, 'manifest.json');
    const manifest = JSON.parse(readFileSync(filename, 'utf8'));
    manifest.checkpoints.find(entry => entry.checkpoint === 'wake-refused').expected_change = true;
    writeFileSync(filename, JSON.stringify(manifest));
    const result = publish(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, new RegExp(`checkpoint ${names.indexOf('wake-refused') + 1} violates`));
  } finally { rmSync(fixture.root, { recursive: true, force: true }); }
});

for (const [label, mutate] of [
  ['Unicode bytes', receipt => { receipt.utf8_bytes = [99,97,102,101]; }],
  ['Host identity', receipt => { receipt.record.host_id = 'other-host'; }],
  ['Plan/Play guest correlation', receipt => { receipt.record.plan_id = 'other-plan'; }],
]) {
  test(`Pages composition rejects ${label} while paired native evidence remains valid`, () => {
    const fixture = makeFixture();
    try {
      const filename = path.join(fixture.evidence, '../memory-lantern-native-edit.json');
      const receipt = JSON.parse(readFileSync(filename, 'utf8'));
      mutate(receipt); writeFileSync(filename, JSON.stringify(receipt));
      const result = publish(fixture);
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /committed text receipt/);
      assert.equal(readFileSync(path.join(fixture.site, 'journeys/index.html'), 'utf8'),
        '<!-- conduit-conduitos-journey-card@1 -->');
    } finally { rmSync(fixture.root, { recursive: true, force: true }); }
  });
}
