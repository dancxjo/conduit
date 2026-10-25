// Synthetic publication custody fixture; not executable platform proof.
import { NATIVE_LOOK_MATRIX } from '../../tools/ci/conduitos-look-matrix.mjs';
export function lookFixture({ commit, image, host, boot }, change = () => {}) {
  const proof = {
    proof_class: 'freestanding-emulator', source_commit: commit, image_sha256: image,
    host_id: host, boot_id: boot, build_id: 'build', image_id: 'image', profile_id: 'profile',
    look_selection: {
      schema: 'conduit.conduitos/local-look-proof@1', status: 'passed', input: 'real-qmp-keyboard',
      source_commit: commit, image_sha256: image,
      body_actions: 0, physical_evidence: false, human_enactment: false,
      face_id: 'face', face_revision: 1,
      same_checked_component: { source_document_id: 'c'.repeat(64), checked_plot_id: 'd'.repeat(64), fore_id: 'e'.repeat(64) },
      same_recipe_canonical_sha256: 'f'.repeat(64), same_layout_canonical_sha256: '1'.repeat(64), transitions: [],
    },
  };
  const manifest = { context: { source_commit: proof.source_commit, image_sha256: proof.image_sha256 }, checkpoints: [] };
  const face = { identity: 'face', revision: 1, basis: { body_id: null }, actions: ['unchanged-action'], text: ['same meaning'] };
  const styles = [null, ...NATIVE_LOOK_MATRIX];
  const chunks = [];
  const native = [];
  for (let i = 0; i < styles.length; i++) {
    const show = {
      face_id: 'face', face_revision: 1, host_id: host, boot_id: boot, offer_generation: 1,
      lifecycle: 'Available', failure: null, plan_id: `plan-${i === 10 ? 0 : i}`,
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
      const [, density, palette, scale, typography, name, capture] = styles[i];
      const transition = {
        schema: 'conduit.conduitos/component-realization@1', selection_entrance: 'local-creche',
        local_scanout_acknowledged: true, owner_show_acknowledged: false,
        previous_show_id: `show-${i - 1}`, show_id: `show-${i}`, face_id: 'face', face_revision: 1,
        host_id: host, boot_id: boot, offer_generation: 1, mask_plan_id: show.plan_id,
        active_play_id: show.active_play_id, manifestation_id: show.manifestation_id,
        expression: { expression: { density, palette, typography, text_scale_percent: scale } }, fallback: null,
        configuration: { root: `root-${i}`, component: structuredClone(proof.look_selection.same_checked_component), recipe_canonical_sha256: proof.look_selection.same_recipe_canonical_sha256, layout_canonical_sha256: proof.look_selection.same_layout_canonical_sha256 },
        render: { commands: 25 },
      };
      proof.look_selection.transitions.push({ ...structuredClone(transition), screenshot_checkpoint: capture ? name : null });
      lines.push(`${'CONDUIT_NATIVE_COMPONENT_REALIZATION '}${JSON.stringify(transition)}\n`);
    }
    chunks.push(lines);
    if (i > 0 && !styles[i][6]) continue;
    const name = i === 0 ? 'creche-ready' : styles[i][5];
    manifest.checkpoints.push({
      chunkIndex: i,
      checkpoint: name, expected_change: i > 0, unchanged: false, health_refusal: null,
      guest_boot_record: { boot_id: boot, host_id: host, build_id: 'build', image_binding: 'image', profile_id: 'profile' },
      frame: { checkpoint: name },
      native_receipts: { latest_acknowledged_face: { face_id: 'face', face_revision: 1, basis: structuredClone(face.basis), show: structuredClone(receipt) } },
    });
  }
  change({ proof, manifest, native, chunks });
  let serial = '';
  chunks.forEach((chunk, index) => {
    serial += chunk.join('');
    const checkpoint = manifest.checkpoints.find(entry => entry.chunkIndex === index);
    if (checkpoint) checkpoint.serial_byte_end ??= Buffer.byteLength(serial);
  });
  for (const entry of manifest.checkpoints) delete entry.chunkIndex;
  return { proof, manifest, serial: Buffer.from(serial), faces: native };
}
