import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validateNativeLookReceipt } from './conduitos-look-receipt.mjs';

function specimen(change = () => {}) {
  const proof = {
    proof_class: 'freestanding-emulator', source_commit: 'a'.repeat(40), image_sha256: 'b'.repeat(64),
    host_id: 'host', boot_id: 'boot', build_id: 'build', image_id: 'image', profile_id: 'profile',
    look_selection: {
      schema: 'conduit.conduitos/local-look-proof@1', status: 'passed', input: 'real-qmp-keyboard',
      body_actions: 0, physical_evidence: false, human_enactment: false,
      face_id: 'face', face_revision: 1,
      same_checked_component: { source_document_id: 'c'.repeat(64), checked_plot_id: 'd'.repeat(64), fore_id: 'e'.repeat(64) },
      same_recipe_canonical_sha256: 'f'.repeat(64), transitions: [],
    },
  };
  const manifest = { context: { source_commit: proof.source_commit, image_sha256: proof.image_sha256 }, checkpoints: [] };
  const face = { identity: 'face', revision: 1, basis: { body_id: null }, actions: ['unchanged-action'], text: ['same meaning'] };
  const names = ['creche-ready', 'creche-editorial-compact', 'creche-editorial-spacious', 'creche-engineering-spacious', 'creche-engineering-compact'];
  const styles = [null, ['compact', 'editorial', 100], ['spacious', 'editorial', 125], ['spacious', 'phosphor', 125], ['compact', 'phosphor', 100]];
  const chunks = [];
  const native = [];
  for (let i = 0; i < 5; i++) {
    const show = {
      face_id: 'face', face_revision: 1, host_id: 'host', boot_id: 'boot', offer_generation: 1,
      lifecycle: 'Available', failure: null, plan_id: `plan-${i === 4 ? 0 : i}`,
      active_play_id: `play-${i}`, manifestation_id: `manifestation-${i}`, play_sequence: i + 1,
    };
    const receipt = {
      ...Object.fromEntries(['face_id', 'face_revision', 'host_id', 'boot_id', 'offer_generation', 'lifecycle', 'manifestation_id'].map(k => [k, show[k]])),
      show_id: `show-${i}`, mask_plan_id: show.plan_id, mask_active_play_id: show.active_play_id,
      mask_plot: { checked_plot_id: 'mask' }, manifestation: show,
    };
    const record = { schema: 'conduit.native-face/v1', face: structuredClone(face), show: receipt, observation_sequence: i + 1, play_sequence: i + 1 };
    native.push(record);
    const lines = [`CONDUIT_NATIVE_FACE ${JSON.stringify(record)}\n`];
    if (i > 0) {
      const [density, palette, scale] = styles[i];
      const transition = {
        schema: 'conduit.conduitos/component-realization@1', selection_entrance: 'local-creche',
        local_scanout_acknowledged: true, owner_show_acknowledged: false,
        previous_show_id: `show-${i - 1}`, show_id: `show-${i}`, face_id: 'face', face_revision: 1,
        host_id: 'host', boot_id: 'boot', offer_generation: 1, mask_plan_id: show.plan_id,
        active_play_id: show.active_play_id, manifestation_id: show.manifestation_id,
        expression: { expression: { density, palette, text_scale_percent: scale } }, fallback: null,
        configuration: { root: `root-${i}`, component: structuredClone(proof.look_selection.same_checked_component), recipe_canonical_sha256: proof.look_selection.same_recipe_canonical_sha256 },
        render: { commands: 25 },
      };
      proof.look_selection.transitions.push(transition);
      lines.push(`${'CONDUIT_NATIVE_COMPONENT_REALIZATION '}${JSON.stringify(transition)}\n`);
    }
    chunks.push(lines);
    manifest.checkpoints.push({
      checkpoint: names[i], expected_change: i > 0, unchanged: false, health_refusal: null,
      guest_boot_record: { boot_id: 'boot', host_id: 'host', build_id: 'build', image_binding: 'image', profile_id: 'profile' },
      frame: { checkpoint: names[i] },
      native_receipts: { latest_acknowledged_face: { face_id: 'face', face_revision: 1, basis: structuredClone(face.basis), show: structuredClone(receipt) } },
    });
  }
  change({ proof, manifest, native, chunks });
  let serial = '';
  chunks.forEach((chunk, index) => {
    serial += chunk.join('');
    manifest.checkpoints[index].serial_byte_end ??= Buffer.byteLength(serial);
  });
  return { proof, manifest, serial: Buffer.from(serial) };
}
const validate = ({ proof, manifest, serial }) => validateNativeLookReceipt(proof, manifest, serial);

test('four captured local Looks retain semantic Face and return to original Mask plan', () => {
  assert.doesNotThrow(() => validate(specimen()));
});

test('receipt cannot reorder, omit or invent serial realizations', () => {
  for (const mutate of [
    ({ proof }) => proof.look_selection.transitions.reverse(),
    ({ proof }) => proof.look_selection.transitions.pop(),
    ({ proof }) => { proof.look_selection.transitions[1].configuration.recipe_canonical_sha256 = '0'.repeat(64); },
    ({ proof }) => { proof.look_selection.transitions[0].boot_id = 'foreign-boot'; },
    ({ chunks }) => chunks[2].pop(),
    ({ chunks }) => chunks[2].push(chunks[1][1]),
  ]) assert.throws(() => validate(specimen(mutate)));
});

test('self-consistent forged receipts still cannot change style, authority, Face or Show custody', () => {
  for (const mutate of [
    transition => { transition.owner_show_acknowledged = true; },
    transition => { transition.local_scanout_acknowledged = false; },
    transition => { transition.previous_show_id = 'stale-show'; },
    transition => { transition.mask_plan_id = 'foreign-plan'; },
    transition => { transition.expression.expression.palette = 'engineering'; },
    transition => { transition.configuration.component.fore_id = '0'.repeat(64); },
    transition => { transition.face_revision = 2; },
    transition => { transition.render.commands = 49; },
  ]) {
    assert.throws(() => validate(specimen(({ proof, chunks }) => {
      mutate(proof.look_selection.transitions[1]);
      chunks[2][1] = `CONDUIT_NATIVE_COMPONENT_REALIZATION ${JSON.stringify(proof.look_selection.transitions[1])}\n`;
    })));
  }
});

test('checkpoint prefixes cannot borrow later publications or hide Body actions', () => {
  for (const mutate of [
    ({ manifest }) => { [manifest.checkpoints[1], manifest.checkpoints[2]] = [manifest.checkpoints[2], manifest.checkpoints[1]]; },
    ({ manifest }) => { manifest.checkpoints[1].serial_byte_end = 1; },
    ({ manifest }) => { manifest.checkpoints[2].native_receipts.latest_acknowledged_face.show.show_id = 'stale'; },
    ({ manifest }) => { manifest.checkpoints[2].guest_boot_record.image_binding = 'another-image'; },
    ({ chunks }) => { chunks[2].push('CONDUIT_NATIVE_BIRTH {"accepted_effects":1}\n'); },
    ({ chunks }) => { chunks[2].push('CONDUIT_NATIVE_ACTION {"accepted_effects":1}\n'); },
    ({ proof }) => { proof.look_selection.body_actions = 1; },
    ({ proof }) => { proof.look_selection.physical_evidence = true; },
    ({ proof }) => { proof.source_commit = '0'.repeat(40); },
    ({ native, chunks }) => {
      native[2].face.text = ['changed semantic meaning'];
      chunks[2][0] = `CONDUIT_NATIVE_FACE ${JSON.stringify(native[2])}\n`;
    },
  ]) assert.throws(() => validate(specimen(mutate)));
});
