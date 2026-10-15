// Documentary integrity tests only; fixtures do not establish live Mask support.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { renderThermostatJourney, validateThermostatJourney } from './pipeline/thermostat-journey.mjs';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
function packet() {
  const root = mkdtempSync(path.join(os.tmpdir(), 'thermostat-documentary-test-'));
  mkdirSync(path.join(root, 'capture'));
  const observation = text => ({ view: { body_id: 'body/test', face_id: 'face/test', face_revision: '1', subjects: [
    { name: 'Target temperature', values: [{ kind: 'value/quantity@1', text }] },
  ] }, owner: { face: { identity: 'face/test', basis: { body_id: 'body/test' } } },
  retained_body_execution: { body_id: 'body/test', plan_id: 'plan/test', active_play_id: 'play/test' } });
  const wav = Buffer.alloc(48);
  wav.write('RIFF'); wav.writeUInt32LE(40, 4); wav.write('WAVEfmt ', 8); wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20); wav.writeUInt16LE(2, 22); wav.writeUInt32LE(48000, 24);
  wav.writeUInt32LE(192000, 28); wav.writeUInt16LE(4, 32); wav.writeUInt16LE(16, 34);
  wav.write('data', 36); wav.writeUInt32LE(4, 40);
  const words = Buffer.from('Settings.');
  const frameFile = { frame_index: 0, path: 'capture/spoken-00.wav', bytes: wav.length, sha256: hash(wav) };
  const receipt = { schema: 'conduit.proof/thermostat-owner-browser@2', owner_source_commit: 'a'.repeat(40),
    browser_runtime_source_commit: 'a'.repeat(40), handbook_ui_source_commit: 'a'.repeat(40), handbook_ui_source_clean: true,
    source_relation: 'exact-source', owner_host_id: 'host/owner', presentation_peer_host_id: 'host/peer', browser_host_id: 'host/browser',
    body_id: 'body/test', body_plan_id: 'plan/test', body_play_id: 'play/test', physical_sensor_or_equipment: false,
    revisions: [observation('21 °C')], stale_show: { accepted: false }, journey: {
      terminal_evidence: Array.from({ length: 2 }, () => ({ body_id: 'body/test', host_id: 'host/peer', face_id: 'face/test', show_state: 'available', owner_selected_show_current: true })),
      terminal_before: { face: { identity: 'face/test' } }, before_terminal: observation('22.5 °C'), terminal_stale: { accepted: false, code: 'stale-face-or-show', before: { face: { properties: [] } }, after: { face: { properties: [] } } },
      browser_stale: { accepted: false, before: { face: { properties: [] } }, after: { face: { properties: [] } } }, terminal_actions: [{ accepted: false }, { accepted: true }, { accepted: true }],
      after_peer_closed: { biography: { body_id: 'body/test' }, realization: { plan: { plan_id: 'plan/test' }, play: { active_play_id: 'play/test' } } }, after_terminal: observation('18 °C'),
      speech: { before: { face: { identity: 'face/test', properties: [] } }, after: { face: { identity: 'face/test', properties: [] } }, accepted: true, speaker_delivery: false, human_listening: false,
        evidence: { route: { body_id: 'body/test', face_id: 'face/test', face_revision: '1', profile: 'hosted-presentation',
          owner_host: { host_id: 'host/owner' }, mask_host: { host_id: 'host/peer' } }, execution: {
          template: { template_identity: 'spoken/control@1', source_face_identity: 'face/test', source_face_revision: '1', wording: words.toString(), wording_digest: [...createHash('sha256').update(words).digest()] },
          source_show: { show_id: 'show/test', show: { host_id: 'host/peer', boot_id: 'boot/test' } },
          completed_wording_bytes: words.length, completed_wording_sha256: [...createHash('sha256').update(words).digest()],
          frames: [{ frame_index: 0, source_face_id: 'face/test', source_show_id: 'show/test', host_id: 'host/peer', boot_id: 'boot/test',
            speech_play_id: 'speech/play', provider_sha256: 'b'.repeat(64), text_bytes: words.length,
            text_sha256: [...createHash('sha256').update(words).digest()], pcm_bytes: 4, artifact_digest_scope: 'wav', artifact_sha256: hash(wav) }] } },
        artifacts: { frames: [frameFile], recording: { path: 'capture/spoken-reading.wav', bytes: wav.length, sha256: hash(wav), source_frames: [frameFile.path] } } },
      lulled: { status: { realization: null }, owner: { face: { actions: [] } } },
      restarted: { ...observation('21 °C'), status: { biography: { body_id: 'body/test' }, realization: { play: { active_play_id: 'play/next' } } } },
    } };
  const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=', 'base64');
  const files = new Map([
    ['receipt.json', Buffer.from(JSON.stringify(receipt))], ['terminal.txt', Buffer.from('18 °C')],
    ['spoken-00.wav', wav], ['spoken-reading.wav', wav],
    ...['desktop', 'raised', 'mobile', 'body-menu', 'after-terminal', 'lulled', 'restarted'].map(id => [`thermostat-${id}.png`, png]),
  ]);
  const manifest = { schema: 'conduit.thermostat-journey/capture@2', result: 'browser-terminal-spoken-complete',
    source_commit: 'a'.repeat(40), capture_tool_commit: 'a'.repeat(40), capture_tool_dirty: false, source_relation: 'exact-source', body_id: 'body/test', body_plan_id: 'plan/test', body_play_id: 'play/test', files: [] };
  function seal() {
    files.set('receipt.json', Buffer.from(JSON.stringify(receipt)));
    manifest.files = [...files].map(([name, bytes]) => {
      writeFileSync(path.join(root, 'capture', name), bytes);
      return { path: `capture/${name}`, bytes: bytes.length, sha256: hash(bytes) };
    });
    writeFileSync(path.join(root, 'manifest.json'), JSON.stringify(manifest));
  }
  seal();
  return { root, receipt, manifest, files, seal };
}
for (const [name, mutate, expected] of [
  ['changed Body in a re-sealed observation', p => { p.receipt.revisions[0].view.body_id = 'body/other'; }, /changed Body or Play/],
  ['changed original Play', p => { p.receipt.revisions[0].retained_body_execution.active_play_id = 'play/other'; }, /changed Body or Play/],
  ['same Play claimed after Lull', p => { p.receipt.journey.restarted.status.realization.play.active_play_id = 'play/test'; }, /lifecycle/],
  ['accepted stale action', p => { p.receipt.journey.terminal_stale.accepted = true; }, /stale-Show/],
  ['missing speech', p => { p.receipt.journey.speech.accepted = false; }, /spoken encounter/],
  ['wrong spoken Host', p => { p.receipt.journey.speech.evidence.execution.frames[0].host_id = 'host/other'; }, /strictly equal/],
  ['wrong spoken Show', p => { p.receipt.journey.speech.evidence.execution.frames[0].source_show_id = 'show/other'; }, /strictly equal/],
  ['changed spoken wording', p => { p.receipt.journey.speech.evidence.execution.template.wording = 'Different.'; }, /strictly equal/],
  ['interaction authority on spoken route', p => { p.receipt.journey.speech.evidence.route.interaction_line = {}; }, /read only/],
  ['wrong spoken revision', p => { p.receipt.journey.speech.evidence.execution.template.source_face_revision = '2'; }, /strictly equal/],
  ['mixed source relation', p => { p.manifest.source_relation = 'development-cross-source'; }, /capture identity/],
  ['wrong spoken Face', p => { p.receipt.journey.speech.evidence.execution.template.source_face_identity = 'other'; }, /strictly equal/],
  ['mixed source', p => { p.receipt.browser_runtime_source_commit = 'b'.repeat(40); }, /receipt identity/],
  ['dirty tool', p => { p.manifest.capture_tool_dirty = true; }, /capture identity/],
  ['changed PCM with resealed inventory', p => { const wav = Buffer.from(p.files.get('spoken-00.wav')); wav[44] = 1; p.files.set('spoken-00.wav', wav); }, /strictly equal/],
  ['changed cross-Mask target', p => { p.receipt.journey.after_terminal.view.subjects[0].values[0].text = '21 °C'; }, /target or reset/],
]) {
  test(`refuses ${name}`, () => {
    const p = packet();
    try { validateThermostatJourney(p.root); mutate(p); p.seal(); assert.throws(() => validateThermostatJourney(p.root), expected); }
    finally { rmSync(p.root, { recursive: true }); }
  });
}
test('refuses changed media bytes without re-sealing', () => {
  const p = packet();
  try {
    writeFileSync(path.join(p.root, 'capture/thermostat-desktop.png'), 'substitute');
    assert.throws(() => validateThermostatJourney(p.root), /digest or bounds/);
  } finally { rmSync(p.root, { recursive: true }); }
});
test('refuses traversal even with a valid external file digest', () => {
  const p = packet();
  try {
    p.manifest.files[0].path = '../receipt.json';
    writeFileSync(path.join(p.root, 'manifest.json'), JSON.stringify(p.manifest));
    assert.throws(() => validateThermostatJourney(p.root), /unsafe or duplicate/);
  } finally { rmSync(p.root, { recursive: true }); }
});

test('renders a documentary fixture with retained audio and a separate Body chapter', async () => {
  const { spawnSync } = await import('node:child_process');
  const { readFileSync, existsSync } = await import('node:fs');
  const { renderThermostatJourney } = await import('./pipeline/thermostat-journey.mjs');
  const p = packet();
  const destination = `${p.root}-render`;
  const previousCwd = process.cwd();
  try {
    const git = args => { const result = spawnSync('git', args, { cwd: p.root, encoding: 'utf8' }); assert.equal(result.status, 0, result.stderr); return result.stdout.trim(); };
    git(['init', '-q']);
    git(['-c', 'user.name=Documentary fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--allow-empty', '-qm', 'Documentary render fixture']);
    const commit = git(['rev-parse', 'HEAD']);
    process.chdir(p.root);
    p.manifest.source_commit = commit; p.manifest.capture_tool_commit = commit;
    for (const key of ['owner_source_commit', 'browser_runtime_source_commit', 'handbook_ui_source_commit']) p.receipt[key] = commit;
    p.seal();
    const result = renderThermostatJourney(p.root, destination, commit, '', '');
    const html = readFileSync(path.join(destination, 'index.html'), 'utf8');
    assert.equal(result.speechAvailable, true);
    assert.match(html, /<audio controls/);
    assert.match(html, /separate Body menu/);
    assert.ok(html.indexOf('id="speech"') < html.indexOf('id="inspect"'));
    assert.ok(existsSync(path.join(destination, 'capture/spoken-reading.wav')));
    assert.ok(!html.includes('speech-refusal.txt'));
  } finally { process.chdir(previousCwd); rmSync(p.root, { recursive: true }); rmSync(destination, { recursive: true, force: true }); }});


test('current thermostat publication uses the canonical Look stylesheet hook', () => {
  const p = packet();
  try {
    const revision = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
    p.manifest.source_commit = revision;
    p.manifest.capture_tool_commit = revision;
    for (const key of ['owner_source_commit', 'browser_runtime_source_commit', 'handbook_ui_source_commit']) p.receipt[key] = revision;
    p.seal();
    const output = path.join(p.root, 'publication');
    const css = readFileSync('targets/browser/host/assets/conduit.css', 'utf8');
    renderThermostatJourney(p.root, output, revision, css, '');
    const html = readFileSync(path.join(output, 'index.html'), 'utf8');
    assert.ok(html.includes('data-application-look="conduit.look/phosphor@1"'));
    assert.ok(html.includes(':where([data-application-look])'));
    assert.ok(!html.includes('data-application-theme='));
  } finally { rmSync(p.root, { recursive: true, force: true }); }
});
