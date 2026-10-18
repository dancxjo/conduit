// Bind the Unicode text claim to an actual record in the captured guest session.
export function validateNativeTextEditReceipt(proof, receipt, serial) {
  const expected = 'café';
  const identities = ['boot_id', 'body_id', 'host_id'];
  const fields = [...identities, 'plan_id', 'active_play_id', 'checked_plot_id', 'result_sign_id', 'result'];
  const record = receipt?.record;
  if (proof.committed_edit_receipt !== 'memory-lantern-native-edit.json'
    || receipt?.schema !== 'conduit.text/native-committed-edit@1'
    || receipt.proof_class !== 'freestanding-emulator'
    || receipt.physical_evidence !== false || receipt.human_enactment !== false
    || receipt.committed_text !== expected
    || JSON.stringify(receipt.utf8_bytes) !== JSON.stringify([...Buffer.from(expected)])
    || record?.result !== expected
    || identities.some(field => record[field] !== proof[field])
    || fields.some(field => typeof record[field] !== 'string' || !record[field])) {
    throw new Error('ConduitOS committed text receipt does not match its exact journey');
  }
  const records = serial.toString('utf8').split('\n')
    .filter(line => line.startsWith('CONDUIT_PRODUCT_JOURNEY '))
    .map(line => JSON.parse(line.slice('CONDUIT_PRODUCT_JOURNEY '.length)));
  if (!records.some(candidate => fields.every(field => candidate[field] === record[field]))) {
    throw new Error('ConduitOS committed text receipt has no matching guest record');
  }
}
