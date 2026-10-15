// Documentary integrity tests only; fixtures do not establish live Mask support.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { validateThermostatJourney } from './pipeline/thermostat-journey.mjs';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
function packet() {
  const root = mkdtempSync(path.join(os.tmpdir(), 'thermostat-documentary-test-'));
  mkdirSync(path.join(root, 'capture'));
  const observation = text => ({ view: { body_id: 'body/test', face_id: 'face/test', subjects: [
    { name: 'Target temperature', values: [{ kind: 'value/quantity@1', text }] },
  ] }, owner: { face: { identity: 'face/test', basis: { body_id: 'body/test' } } },
  retained_body_execution: { body_id: 'body/test', plan_id: 'plan/test', active_play_id: 'play/test' } });
  const receipt = { schema: 'conduit.proof/thermostat-owner-browser@1', owner_source_commit: 'a'.repeat(40),
    body_id: 'body/test', body_plan_id: 'plan/test', body_play_id: 'play/test', physical_sensor_or_equipment: false,
    revisions: [observation('21 °C')], stale_show: { accepted: false }, journey: {
      terminal_evidence: Array.from({ length: 2 }, () => ({ body_id: 'body/test', show_state: 'available', owner_selected_show_current: true })),
      terminal_stale: { accepted: false }, after_terminal: observation('18 °C'),
      speech: { accepted: false, exit_code: 1 },
      lulled: { status: { realization: { play: { active_play_id: null } } }, owner: { face: { actions: [] } } },
      restarted: { ...observation('21 °C'), status: { biography: { body_id: 'body/test' }, realization: { play: { active_play_id: 'play/next' } } } },
    } };
  const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=', 'base64');
  const files = new Map([
    ['receipt.json', Buffer.from(JSON.stringify(receipt))], ['terminal.txt', Buffer.from('18 °C')],
    ['speech-refusal.txt', Buffer.from('selected speech requires an idle installed Body owner')],
    ...['desktop', 'raised', 'after-terminal', 'lulled', 'restarted'].map(id => [`thermostat-${id}.png`, png]),
  ]);
  const manifest = { schema: 'conduit.thermostat-journey/capture@1', result: 'browser-terminal-complete-speech-unavailable',
    source_commit: 'a'.repeat(40), body_id: 'body/test', body_plan_id: 'plan/test', body_play_id: 'play/test', files: [] };
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
  ['invented speech success', p => { p.receipt.journey.speech.accepted = true; }, /speech refusal/],
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
