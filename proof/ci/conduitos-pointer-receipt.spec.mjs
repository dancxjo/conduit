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
