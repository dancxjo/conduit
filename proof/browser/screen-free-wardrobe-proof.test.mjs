import assert from 'node:assert/strict';
import test from 'node:test';
import { screenFreeWardrobeCommands, verifyScreenFreeWardrobe } from './screen-free-wardrobe-proof.mjs';

const report = (revision, worn, preference) =>
  `Owner wardrobe revision ${revision}. Worn: ${worn}. Preference: ${preference}. Selected: route/terminal. Current acknowledged Show: show/terminal. Planning: NotRequired.\n` +
  `Current Mask wardrobe\nterminal on Host host/owner: available. Route route/terminal.\n` +
  `Text Face revision=${revision} Show=show/reading/${revision}\n`;

test('wardrobe proof requires actual refusal, unchanged revision, and ordered owner changes', () => {
  const outputs = [report(7, 'terminal', 'none'),
    'Wardrobe refused: Mask absent-mask is not in the current owner Plan. Enter wardrobe to inspect current owner state.\n',
    report(7, 'terminal', 'none'), report(8, 'none', 'none'),
    report(9, 'terminal', 'none'), report(10, 'terminal', 'terminal'),
    report(10, 'terminal', 'terminal')];
  const session = { commands: screenFreeWardrobeCommands,
    responses: outputs.map((output, index) => ({
      command: screenFreeWardrobeCommands[index], output,
    })), transcript: 'Continuing retained Body body/test' };
  const receipt = verifyScreenFreeWardrobe(session,
    { host_id: 'host/owner', boot_id: 'boot/owner' }, false);
  assert.equal(receipt.inspect.wardrobe_revision, '7');
  assert.equal(receipt.failure.outcome, 'refused');
  assert.equal(receipt.final.wardrobe_revision, '10');
  const wrong = structuredClone(session);
  wrong.responses[2].output = report(8, 'terminal', 'none');
  assert.throws(() => verifyScreenFreeWardrobe(wrong,
    { host_id: 'host/owner', boot_id: 'boot/owner' }, false),
  /refused Mask name must not mutate/);
});
