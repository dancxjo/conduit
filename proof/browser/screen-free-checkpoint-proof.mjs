// Interpret only the public nonvisual owner's current Face and wardrobe
// report while the other producer holds an environmental loss/recovery phase.
import assert from 'node:assert/strict';

export const checkpointPhases = [
  'model-provider-unavailable', 'model-provider-restored',
  'browser-presentation-unavailable', 'browser-presentation-restored',
];

export function verifyCheckpointReady(ready, phase, bodyId, ownerPart, face) {
  assert.equal(ready.schema, 'conduit.proof/screen-free-checkpoint@1');
  assert.equal(ready.phase, phase);
  assert.equal(ready.body_id, bodyId);
  assert.equal(ready.owner_host_id, ownerPart.host_id);
  assert.equal(ready.owner_boot_id, ownerPart.boot_id);
  assert.equal(ready.face_id, face.presentation.identity);
  assert.equal(String(ready.face_revision), face.presentation_revision_decimal);
  assert.equal(face.presentation.basis.body_id, bodyId);
  assert.equal(face.advertisement.host_id, ownerPart.host_id);
  assert.equal(face.advertisement.boot_id, ownerPart.boot_id);
  assert.match(ready.route_id, /^route\//);
  assert.equal(typeof ready.route_available, 'boolean');
  assert.equal(ready.route_available, phase.endsWith('-restored'));
}

export function verifyCheckpointWardrobe(output, ready, ownerPart, selectedSpeaker) {
  const refusal = output.split('\n').flatMap(line => {
    const start = line.indexOf('{"');
    if (start < 0) return [];
    try { return [JSON.parse(line.slice(start))]; } catch { return []; }
  }).find(item => item.schema === 'conduit.body/screen-free-wardrobe-refusal@1');
  if (ready.phase === 'browser-presentation-unavailable' && refusal) {
    assert.equal(ready.route_available, false);
    assert.equal(refusal.code, 'stale-body-or-face');
    assert.match(output, /previous Mask Plan is stale, so no route from that Plan can be used now/);
    assert.doesNotMatch(output, /Owner wardrobe revision \d+\./,
      'a stale Plan cannot be reported as the current wardrobe');
    return { wardrobe_revision: null, route_id: ready.route_id,
      route_available: false, route_announcement: 'previous Mask Plan is stale',
      wardrobe_selected_playback_receipts: 0, refusal_code: refusal.code };
  }
  const revision = /Owner wardrobe revision (\d+)\./.exec(output)?.[1];
  assert.ok(revision, `${ready.phase} lacks current owner wardrobe revision`);
  if (ready.wardrobe_revision !== undefined && ready.wardrobe_revision !== null) {
    assert.equal(revision, String(ready.wardrobe_revision));
  }
  const route = output.split('\n').find(line =>
    line.includes(`Route ${ready.route_id}.`));
  assert.ok(route, `${ready.phase} lacks its exact owner route`);
  const expected = ready.route_available ? 'available' : 'unavailable';
  assert.ok(route.includes(`: ${expected}. Route ${ready.route_id}.`),
    `${ready.phase} did not announce the exact ${expected} route`);
  assert.match(output, /Current Mask wardrobe/);
  const receipts = output.split('\n').flatMap(line => {
    const start = line.indexOf('{"');
    if (start < 0) return [];
    try { return [JSON.parse(line.slice(start))]; } catch { return []; }
  });
  const plays = receipts.filter(item => item.schema === 'conduit.body/spoken-face-playback@1');
  const turns = receipts.filter(item => item.schema === 'conduit.body/spoken-face-turn@1');
  if (selectedSpeaker) {
    assert.ok(plays.length > 0 && turns.length > 0,
      `${ready.phase} lacks selected speaker wardrobe Play`);
    assert.ok(turns.some(turn => turn.outcome === 'Completed' &&
      turn.face_revision_decimal === revision),
    `${ready.phase} lacks completed current wardrobe turn`);
    for (const play of plays) {
      assert.equal(play.outcome, 'Completed');
      assert.equal(play.host_id, ownerPart.host_id);
      assert.equal(play.boot_id, ownerPart.boot_id);
      assert.equal(play.face_revision_decimal, revision);
      assert.ok(play.same_play_capture?.wav_sha256,
        `${ready.phase} speaker Play lacks same-Play capture`);
      assert.ok(turns.some(turn => turn.face_id === play.face_id &&
        turn.face_revision_decimal === play.face_revision_decimal &&
        turn.source_show_id === play.source_show_id));
    }
  } else {
    assert.equal(plays.length, 0);
    assert.equal(turns.length, 0);
    assert.ok(output.includes(`Text Face revision=${revision} Show=`),
      `${ready.phase} lacks current text wardrobe readout`);
  }
  return { wardrobe_revision: revision, route_id: ready.route_id,
    route_available: ready.route_available,
    route_announcement: route.trim(),
    wardrobe_selected_playback_receipts: plays.length };
}
