// Capture the installed owner's selected speaker Play through the same browser
// carrier and acknowledged graphical Show used for the three-host journey.
import assert from 'node:assert/strict';

const RESPONSE = 'selected-speech-response';

export function assertOwnerSelectedSpeech(receipt, { face, bodyId, ownerHostId,
  ownerBootId, operationId, providerSha256 }) {
  assert.equal(receipt.schema, 'conduit.body/selected-speech-terminal@1');
  assert.equal(receipt.operation_id, operationId);
  assert.equal(receipt.outcome, 'completed');
  assert.equal(receipt.face_id, face.face_id);
  assert.equal(String(receipt.face_revision), face.face_revision);
  assert.equal(face.body_id, bodyId);
  assert.equal(face.route.owner_host_id, ownerHostId);
  assert.equal(face.route.owner_boot_id, ownerBootId);
  assert.equal(receipt.source_show_id, face.show_id);
  assert.equal(receipt.source_show_still_current, true);
  assert.equal(receipt.host_id, ownerHostId);
  assert.equal(receipt.boot_id, ownerBootId);
  assert.equal(receipt.provider_sha256, providerSha256);
  assert.ok(Number.isSafeInteger(receipt.offer_generation));
  assert.ok(typeof receipt.selected_resource_pool_id === 'string' && receipt.selected_resource_pool_id);
  assert.ok(typeof receipt.authority_grant_id === 'string' && receipt.authority_grant_id);
  assert.ok(Array.isArray(receipt.batches) && receipt.batches.length > 0);
  for (const batch of receipt.batches) {
    assert.ok(typeof batch.stream_identity === 'string' && batch.stream_identity);
    assert.ok(typeof batch.plan_id === 'string' && batch.plan_id);
    assert.ok(typeof batch.play_id === 'string' && batch.play_id);
    assert.match(batch.source_segments_sha256, /^[0-9a-f]{64}$/);
    assert.equal(batch.provider_sha256, providerSha256);
    assert.ok(Number.isSafeInteger(batch.speaker_blocks_committed)
      && batch.speaker_blocks_committed > 0);
    assert.equal(batch.outcome, 'completed');
  }
  return {
    proof_class: 'installed-owner-selected-speaker-playback',
    body_id: bodyId,
    face_id: face.face_id,
    face_revision: face.face_revision,
    source_show_id: face.show_id,
    browser_route_plan_id: face.route.plan_id,
    browser_mask_plan_id: face.mask_plan_id,
    browser_mask_play_id: face.mask_play_id,
    owner_host_id: ownerHostId,
    owner_boot_id: ownerBootId,
    operation_id: operationId,
    provider_sha256: providerSha256,
    terminal_receipt: receipt,
    batches: receipt.batches,
    speaker_playback_reported: true,
    wav_artifact_from_this_play: false,
    human_hearing_observed: false,
  };
}

export function observeOwnerSpeech(page) {
  const replies = [];
  const listeners = new Map();
  const waiters = new Set();
  const listen = socket => {
    const received = ({ payload }) => {
      try {
        const frame = JSON.parse(String(payload));
        if (frame.kind === RESPONSE) {
          replies.push(frame);
          for (const notify of waiters) notify();
        }
      } catch { /* Other protocol frames are outside this capture. */ }
    };
    listeners.set(socket, received);
    socket.on('framereceived', received);
  };
  page.on('websocket', listen);
  return {
    replies,
    async nextReply(after) {
      if (replies.length > after) return replies[after];
      await new Promise((resolve, reject) => {
        const timer = setTimeout(() => { waiters.delete(notify); reject(new Error('selected speech reply deadline')); }, 10_000);
        const notify = () => {
          if (replies.length <= after) return;
          clearTimeout(timer);
          waiters.delete(notify);
          resolve();
        };
        waiters.add(notify);
      });
      return replies[after];
    },
    close() {
      page.off('websocket', listen);
      for (const [socket, received] of listeners) socket.off('framereceived', received);
    },
  };
}

export async function captureOwnerSelectedSpeech(page, observer, expected) {
  const start = page.getByRole('button', { name: 'Read this view aloud' });
  assert.equal(await start.isEnabled(), true, 'current Show must admit speech');
  let previous = observer.replies.length;
  await start.click();
  const begun = await observer.nextReply(previous);
  assert.equal(begun.outcome, 'started', 'owner must start selected speech');
  assert.ok(begun.operation_id, 'selected speech start needs its operation receipt');
  const operationId = begun.operation_id;
  await page.getByText('The owner started reading this Show.', { exact: false }).waitFor();
  let terminal;
  for (let attempt = 0; attempt < 120 && !terminal; attempt += 1) {
    previous = observer.replies.length;
    await page.getByRole('button', { name: 'Check reading' }).click();
    const reply = await observer.nextReply(previous);
    assert.equal(reply.outcome, 'status');
    assert.equal(reply.operation_id, operationId);
    if (reply.status?.schema === 'conduit.body/selected-speech-terminal@1') {
      terminal = reply.status;
      break;
    }
    assert.equal(reply.status?.state, 'running');
    await page.waitForTimeout(250);
  }
  assert.ok(terminal, 'selected owner speech needs an exact terminal receipt');
  await page.getByText('Owner reading ended: completed.', { exact: true }).waitFor();
  return assertOwnerSelectedSpeech(terminal, { ...expected, operationId });
}
