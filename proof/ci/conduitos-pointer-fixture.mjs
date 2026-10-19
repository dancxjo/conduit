// Structural publication fixture only. This is not physical or QEMU evidence.
export function pointerFixture() {
  const source_changes = Array.from({ length: 4 }, (_, index) => ({
    schema: 'conduit.creche-change/v1', disposition: 'source-accepted-and-shown',
    before_face_id: `creche/face/${index}`, before_revision: index,
    before_show_id: `creche/show/${index}`,
    after_face_id: `creche/face/${index + 1}`, after_revision: index + 1,
    after_show_id: `creche/show/${index + 1}`,
    interaction: { action_id: index < 2 ? 'creche.plot.0' : 'creche.plot.3',
      face_id: `creche/face/${index}`, face_revision: index, show_id: `creche/show/${index}`,
      arguments: [{ name: 'value', value_kind: 'value/bool', value: [index % 2] }] },
  }));
  const sample = (sequence, revision, down, edge, delta = 0) => ({
    schema: 'conduit.native-pointer/v1', sequence,
    face_id: `creche/face/${revision}`, face_revision: revision, show_id: `creche/show/${revision}`,
    primary_pressed: down, press_edge: edge, delta_x: delta, delta_y: 0,
    position_x: 500_000, position_y: 500_000, display_x: 40, display_y: 25, coalesced: 0, dropped: 0,
  });
  const presses = [sample(1, 2, true, true), sample(4, 3, true, true)];
  const releases = [sample(3, 3, false, false), sample(5, 4, false, false)];
  const held = sample(2, 3, true, false, 4_000);
  const focuses = [2, 3].map(revision => ({ action_id: 'creche.plot.3', argument_name: null,
    face_id: `creche/face/${revision}`, face_revision: revision, show_id: `creche/show/${revision}`,
    hit_bounds: { x: 30, y: 10, width: 200, height: 40 },
    display_bounds: { x: 0, y: 0, width: 640, height: 480 } }));
  const receipt = { schema: 'conduit.creche/native-pointer-proof@1', status: 'passed',
    entrance: 'normal-product-creche', proof_class: 'freestanding-emulator',
    target_action: 'creche.plot.3', target_geometry: 'observed-current-frame-hit-bounds',
    keyboard_action_count: 2, pointer_action_count: 2, source_changes, presses, releases,
    held_motion_after_fresh_show: held, held_motion_duplicate_effects: 0, held_motion_on_refreshed_control: true,
    physical_evidence: false, human_enactment: false };
  const lines = (prefix, values) => values.map(value => prefix + JSON.stringify(value) + '\n').join('');
  return { receipt, serial: lines('CONDUIT_CRECHE_FOCUS ', focuses)
    + lines('CONDUIT_NATIVE_POINTER ', [presses[0], held, releases[0], presses[1], releases[1]])
    + lines('CONDUIT_CRECHE_CHANGE ', source_changes) };
}
