import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validateNativeTextEditReceipt } from './conduitos-text-edit-receipt.mjs';
const record = { boot_id: 'boot', body_id: 'body', host_id: 'host', plan_id: 'plan', active_play_id: 'play', checked_plot_id: 'plot', result_sign_id: 'sign', result: 'café' };
const proof = { boot_id: 'boot', body_id: 'body', host_id: 'host', committed_edit_receipt: 'memory-lantern-native-edit.json' };
const receipt = { schema: 'conduit.text/native-committed-edit@1', proof_class: 'freestanding-emulator', physical_evidence: false, human_enactment: false, committed_text: 'café', utf8_bytes: [99,97,102,195,169], record };
const serial = Buffer.from(`CONDUIT_PRODUCT_JOURNEY ${JSON.stringify(record)}\n`);
test('the Unicode claim requires exact bytes and a captured Plan/Play record', () => {
  assert.doesNotThrow(() => validateNativeTextEditReceipt(proof, receipt, serial));
  for (const changed of [
    { ...receipt, utf8_bytes: [99,97,102,101] },
    { ...receipt, physical_evidence: true },
    { ...receipt, record: { ...record, plan_id: 'other' } },
    { ...receipt, record: { ...record, boot_id: 'other' } },
  ]) assert.throws(() => validateNativeTextEditReceipt(proof, changed, serial));
  assert.throws(() => validateNativeTextEditReceipt(proof, receipt, Buffer.from('unrelated session')));
});
