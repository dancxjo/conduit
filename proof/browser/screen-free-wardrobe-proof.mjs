// Verify each command against the live owner's terminal report and the
// screen-free Mask reading that followed it. No wardrobe state is authored here.
import assert from 'node:assert/strict';

const commands = ['wardrobe', 'wardrobe wear absent-mask', 'wardrobe',
  'wardrobe doff terminal', 'wardrobe wear terminal',
  'wardrobe prefer terminal', 'wardrobe'];

const receipts = output => output.split('\n').flatMap(line => {
  const start = line.indexOf('{"');
  if (start < 0) return [];
  try { return [JSON.parse(line.slice(start))]; } catch { return []; }
});

function readout(response, name, ownerPart, selectedSpeaker) {
  const report = [...response.matchAll(/Owner wardrobe revision (\d+)\. Worn: ([^.]+)\. Preference: ([^.]+)\. Selected: ([^.]+)\. Current acknowledged Show: ([^.]+)\. Planning: ([^.]+)\./g)];
  assert.equal(report.length, 1, `${name} needs one owner wardrobe report`);
  const [, revisionText, worn, preference, selected, acknowledgedShow, planning] = report[0];
  const revision = BigInt(revisionText);
  assert.match(response, /Current Mask wardrobe/,
    `${name} needs its owner-authored wardrobe Face`);
  assert.ok(response.includes('terminal on Host'), `${name} lacks admitted terminal route`);
  const found = receipts(response);
  const turns = found.filter(item => item.schema === 'conduit.body/spoken-face-turn@1');
  const plays = found.filter(item => item.schema === 'conduit.body/spoken-face-playback@1');
  let reading;
  if (selectedSpeaker) {
    assert.ok(turns.length > 0 && plays.length > 0,
      `${name} lacks selected speaker Play and terminal turn`);
    for (const play of plays) {
      assert.equal(play.outcome, 'Completed');
      assert.equal(play.speaker_lifecycle, 'StoppedClosed');
      assert.equal(play.host_id, ownerPart.host_id);
      assert.equal(play.boot_id, ownerPart.boot_id);
      assert.equal(play.face_revision_decimal, revisionText);
      assert.ok(play.speaker_frames_committed > 0);
      assert.equal(play.speaker_underruns, 0);
    }
    for (const turn of turns) {
      assert.equal(turn.outcome, 'Completed');
      assert.equal(turn.face_revision_decimal, revisionText);
      assert.ok(plays.some(play => play.face_id === turn.face_id &&
        play.source_show_id === turn.source_show_id &&
        play.face_revision_decimal === turn.face_revision_decimal));
    }
    const turn = turns.at(-1);
    reading = { outcome: turn.outcome, face_id: turn.face_id,
      face_revision: turn.face_revision_decimal, source_show_id: turn.source_show_id,
      completed_segments: turn.completed_segments, selected_playback_receipts: plays.length };
  } else {
    assert.equal(turns.length, 0);
    assert.equal(plays.length, 0);
    const readouts = [...response.matchAll(/Text Face revision=(\d+) Show=(\S+)/g)];
    assert.ok(readouts.length > 0, `${name} lacks text Face readout`);
    const last = readouts.at(-1);
    assert.equal(last[1], revisionText);
    reading = { outcome: 'text-readout', face_revision: last[1],
      source_show_id: last[2] };
  }
  return { wardrobe_revision: revisionText, worn, preference, selected,
    acknowledged_show_id: acknowledgedShow, planning, reading, revision };
}

export function verifyScreenFreeWardrobe(session, ownerPart, selectedSpeaker) {
  assert.deepEqual(session.commands, [...commands, 'quit']);
  assert.deepEqual(session.responses.map(item => item.command), commands);
  const first = readout(session.responses[0].output, 'inspect', ownerPart, selectedSpeaker);
  const failure = session.responses[1].output;
  assert.match(failure, /Wardrobe refused: Mask absent-mask is not in the current owner Plan/);
  assert.doesNotMatch(failure, /Owner wardrobe revision/);
  assert.doesNotMatch(failure, /conduit\.body\/spoken-face-playback@1/);
  const recovered = readout(session.responses[2].output, 'reinspect', ownerPart, selectedSpeaker);
  assert.equal(recovered.revision, first.revision,
    'a refused Mask name must not mutate the owner wardrobe');
  const doff = readout(session.responses[3].output, 'doff', ownerPart, selectedSpeaker);
  assert.equal(doff.revision, first.revision + 1n);
  assert.doesNotMatch(doff.worn, /(?:^|, )terminal(?:, |$)/,
    'doffing terminal must remove that Mask, even when another local Mask remains worn');
  const wear = readout(session.responses[4].output, 'wear', ownerPart, selectedSpeaker);
  assert.equal(wear.revision, doff.revision + 1n);
  assert.match(wear.worn, /terminal/);
  const prefer = readout(session.responses[5].output, 'prefer', ownerPart, selectedSpeaker);
  assert.equal(prefer.revision, wear.revision + 1n);
  assert.match(prefer.preference, /terminal/);
  const final = readout(session.responses[6].output, 'final inspect', ownerPart, selectedSpeaker);
  assert.equal(final.revision, prefer.revision);
  assert.equal(final.worn, prefer.worn);
  assert.equal(final.preference, prefer.preference);
  assert.ok(session.transcript.includes('Continuing retained Body'));
  const keep = ({ revision, ...receipt }) => receipt;
  return { inspect: keep(first), failure: {
    command: commands[1], outcome: 'refused', reason: 'Mask absent-mask is not in the current owner Plan',
    wardrobe_revision_unchanged: true },
  recovery: keep(recovered), doff: keep(doff), wear: keep(wear),
  prefer: keep(prefer), final: keep(final) };
}

export const screenFreeWardrobeCommands = [...commands, 'quit'];
