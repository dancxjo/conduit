import assert from 'node:assert/strict';

export function makeZeroBodyReceipt(installation, liveServiceStatus,
  biographyPresent, pendingBirthTransactionPresent) {
  assert.equal(installation.body_state, null, 'installation already owns a Body');
  // Older installed-host JSON omits the optional field when no Body was joined.
  assert.equal(installation.joined_body_state ?? null, null,
    'installation already joined a Body');
  assert.equal(liveServiceStatus.body_id, null, 'live service already owns a Body');
  assert.equal(liveServiceStatus.host_id, installation.host_id);
  assert.equal(biographyPresent, false, 'biography already exists');
  assert.equal(pendingBirthTransactionPresent, false, 'pending Birth requires recovery');
  return { installation, live_service_status: liveServiceStatus,
    biography_present: biographyPresent,
    pending_birth_transaction_present: pendingBirthTransactionPresent };
}
