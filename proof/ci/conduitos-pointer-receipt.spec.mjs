import assert from 'node:assert/strict';
import test from 'node:test';
import { validateNativePointerReceipt } from '../../tools/ci/conduitos-pointer-receipt.mjs';
import { pointerFixture } from './conduitos-pointer-fixture.mjs';

test('pointer publication retains exact physical and Source receipt custody', () => {
  const { receipt, serial } = pointerFixture();
  assert.doesNotThrow(() => validateNativePointerReceipt(receipt, serial));
});
for (const [label, mutate] of [
  ['Source effect', r => { r.source_changes[2].interaction.arguments[0].value = [1]; }],
  ['Show', r => { r.presses[0].show_id = 'other'; }],
  ['held activation', r => { r.held_motion_after_fresh_show.press_edge = true; }],
  ['missing release', r => { r.releases.pop(); }],
  ['pointer provenance', r => { r.physical_evidence = true; }],
]) {
  test(`pointer publication refuses changed ${label}`, () => {
    const { receipt, serial } = pointerFixture();
    mutate(receipt);
    assert.throws(() => validateNativePointerReceipt(receipt, serial), /pointer receipt differs/);
  });
}
test('pointer publication refuses a changed physical transcript', () => {
  const { receipt, serial } = pointerFixture();
  assert.throws(() => validateNativePointerReceipt(receipt, serial.replace('"press_edge":true', '"press_edge":false')), /pointer receipt differs/);
});

test('pointer publication refuses missing target geometry in the transcript', () => {
  const { receipt, serial } = pointerFixture();
  assert.throws(() => validateNativePointerReceipt(receipt, serial.replace('"width":200', '"width":null')), /pointer receipt differs/);
});

test('held motion must reach the refreshed control even when custody is otherwise intact', () => {
  const { receipt, serial } = pointerFixture();
  const changed = serial.split('\n').map(line => {
    if (!line.startsWith('CONDUIT_NATIVE_POINTER ')) return line;
    const sample = JSON.parse(line.slice('CONDUIT_NATIVE_POINTER '.length));
    if (sample.sequence === 2) sample.display_y = 156;
    return 'CONDUIT_NATIVE_POINTER ' + JSON.stringify(sample);
  }).join('\n');
  receipt.held_motion_after_fresh_show.display_y = 156;
  assert.throws(() => validateNativePointerReceipt(receipt, changed), /pointer receipt differs/);
});


test('release activation retains original Show until the completed tap and owner change', () => {
  const { receipt, serial } = pointerFixture(true);
  assert.doesNotThrow(() => validateNativePointerReceipt(receipt, serial));
});

test('release activation refuses owner mutation before release', () => {
  const { receipt, serial } = pointerFixture(true);
  const lines = serial.trimEnd().split('\n');
  const change = lines.findIndex(line => line.startsWith('CONDUIT_CRECHE_CHANGE ')
    && JSON.parse(line.slice('CONDUIT_CRECHE_CHANGE '.length)).before_revision === 2);
  const [premature] = lines.splice(change, 1);
  const release = lines.findIndex(line => line.startsWith('CONDUIT_NATIVE_POINTER ')
    && JSON.parse(line.slice('CONDUIT_NATIVE_POINTER '.length)).sequence === 3);
  lines.splice(release, 0, premature);
  assert.throws(() => validateNativePointerReceipt(receipt, lines.join('\n') + '\n'), /pointer receipt differs/);
});

test('release activation refuses cancelled movement even inside the target', () => {
  const { receipt, serial } = pointerFixture(true);
  receipt.held_motion_before_release.display_x += 9;
  const changed = serial.split('\n').map(line => {
    if (!line.startsWith('CONDUIT_NATIVE_POINTER ')) return line;
    const sample = JSON.parse(line.slice('CONDUIT_NATIVE_POINTER '.length));
    if (sample.sequence === 2) sample.display_x += 9;
    return 'CONDUIT_NATIVE_POINTER ' + JSON.stringify(sample);
  }).join('\n');
  assert.throws(() => validateNativePointerReceipt(receipt, changed), /pointer receipt differs/);
});
