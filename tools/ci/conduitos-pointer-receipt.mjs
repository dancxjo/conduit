import { isDeepStrictEqual } from 'node:util';

// Publication checks custody of actual records. Rust and the QEMU journey own
// semantic admission; this validator cannot upgrade synthetic records to it.
export function validateNativePointerReceipt(receipt, serial) {
  const fail = () => { throw new Error('ConduitOS pointer receipt differs from the enacted physical gesture and Source records'); };
  const lines = serial.split('\n');
  lines.pop(); // A partial final record is never evidence.
  const records = prefix => lines.filter(line => line.startsWith(prefix)).map(line => JSON.parse(line.slice(prefix.length)));
  const changes = records('CONDUIT_CRECHE_CHANGE ');
  const samples = records('CONDUIT_NATIVE_POINTER ');
  const focuses = records('CONDUIT_CRECHE_FOCUS ');
  const same = (a, b) => isDeepStrictEqual(a, b);
  const has = sample => samples.some(actual => same(actual, sample));
  const lineOf = (prefix, value) => lines.findIndex(line => line.startsWith(prefix)
    && same(JSON.parse(line.slice(prefix.length)), value));
  const insideHit = (point, focus) => {
    const hit = focus?.hit_bounds;
    const extent = focus?.display_bounds;
    return hit && extent && ['x', 'y', 'width', 'height'].every(field => Number.isSafeInteger(hit[field]) && Number.isSafeInteger(extent[field]))
      && extent.x === 0 && extent.y === 0 && extent.width > 0 && extent.height > 0
      && extent.width <= 16384 && extent.height <= 16384 && hit.x >= 0 && hit.y >= 0
      && hit.width > 0 && hit.height > 0 && hit.x + hit.width <= extent.width && hit.y + hit.height <= extent.height
      && Number.isSafeInteger(point.display_x) && Number.isSafeInteger(point.display_y)
      && point.display_x >= hit.x && point.display_x < hit.x + hit.width
      && point.display_y >= hit.y && point.display_y < hit.y + hit.height;
  };

  const releaseActivation = receipt?.schema === 'conduit.creche/native-pointer-proof@2';
  const basis = releaseActivation ? 'before' : 'after';
  const heldRecord = releaseActivation ? 'held_motion_before_release' : 'held_motion_after_fresh_show';
  const heldControl = releaseActivation ? 'held_motion_on_pending_control' : 'held_motion_on_refreshed_control';
  if ((!releaseActivation && receipt?.schema !== 'conduit.creche/native-pointer-proof@1') || receipt.status !== 'passed'
    || receipt.entrance !== 'normal-product-creche' || receipt.proof_class !== 'freestanding-emulator'
    || receipt.target_action !== 'creche.plot.3' || receipt.target_geometry !== 'observed-current-frame-hit-bounds'
    || receipt.keyboard_action_count !== 2 || receipt.pointer_action_count !== 2
    || receipt[heldControl] !== true || receipt.held_motion_duplicate_effects !== 0 || receipt.physical_evidence !== false
    || receipt.human_enactment !== false || changes.length !== 4 || !same(receipt.source_changes, changes)
    || receipt.presses?.length !== 2 || receipt.releases?.length !== 2) fail();
  let previous = 0;
  for (const sample of samples) {
    if (sample.schema !== 'conduit.native-pointer/v1'
      || !Number.isSafeInteger(sample.sequence) || sample.sequence <= previous
      || sample.coalesced !== 0 || sample.dropped !== 0) fail();
    previous = sample.sequence;
  }
  for (const [index, change] of changes.entries()) {
    const interaction = change.interaction;
    if (change.schema !== 'conduit.creche-change/v1' || change.disposition !== 'source-accepted-and-shown'
      || interaction?.action_id !== (index < 2 ? 'creche.plot.0' : 'creche.plot.3')
      || interaction.face_id !== change.before_face_id || interaction.face_revision !== change.before_revision
      || interaction.show_id !== change.before_show_id || interaction.arguments?.length !== 1
      || interaction.arguments[0].value_kind !== 'value/bool'
      || !same(interaction.arguments[0].value, [index % 2])
      || change.after_revision <= change.before_revision || change.before_show_id === change.after_show_id) fail();
  }
  for (let index = 0; index < 2; index++) {
    const change = changes[index + 2];
    const press = receipt.presses[index];
    const release = receipt.releases[index];
    const focus = focuses.find(record => record.action_id === receipt.target_action && record.argument_name === null
      && record.face_id === change.before_face_id && record.show_id === change.before_show_id);
    if (!has(press) || !has(release) || press.primary_pressed !== true || press.press_edge !== true
      || press.face_id !== change.before_face_id || press.face_revision !== change.before_revision
      || press.show_id !== change.before_show_id || release.primary_pressed !== false || release.press_edge !== false
      || release.face_id !== change[`${basis}_face_id`] || release.face_revision !== change[`${basis}_revision`] || release.show_id !== change[`${basis}_show_id`]
      || release.sequence <= press.sequence || focus?.face_revision !== change.before_revision || !insideHit(press, focus) || (releaseActivation && !insideHit(release, focus))) fail();
    if (releaseActivation) {
      const pressLine = lineOf('CONDUIT_NATIVE_POINTER ', press);
      const releaseLine = lineOf('CONDUIT_NATIVE_POINTER ', release);
      const changeLine = lineOf('CONDUIT_CRECHE_CHANGE ', change);
      if (!(pressLine < releaseLine && releaseLine < changeLine)) fail();
      for (const sample of samples.filter(sample => sample.sequence >= press.sequence && sample.sequence <= release.sequence)) {
        if (sample.face_id !== change.before_face_id || sample.face_revision !== change.before_revision
          || sample.show_id !== change.before_show_id || !insideHit(sample, focus)
          || Math.abs(sample.display_x - press.display_x) > 8 || Math.abs(sample.display_y - press.display_y) > 8) fail();
      }
    }
  }
  const held = receipt[heldRecord];
  const heldFocus = focuses.find(record => record.action_id === receipt.target_action && record.argument_name === null
    && record.face_id === changes[2][`${basis}_face_id`] && record.face_revision === changes[2][`${basis}_revision`]
    && record.show_id === changes[2][`${basis}_show_id`]);

  if (!has(held) || held.primary_pressed !== true || held.press_edge !== false
    || held.face_id !== changes[2][`${basis}_face_id`] || held.face_revision !== changes[2][`${basis}_revision`] || held.show_id !== changes[2][`${basis}_show_id`]
    || held.sequence <= receipt.presses[0].sequence || held.sequence >= receipt.releases[0].sequence
    || !insideHit(held, heldFocus) || !(held.delta_x || held.delta_y) || receipt.presses[1].sequence <= receipt.releases[0].sequence) fail();
}
