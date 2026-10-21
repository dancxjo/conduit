// Synthetic publication custody fixture; not executable platform proof.
export function lookFixture({ commit, image, host, boot }) {
  const proof = {
    proof_class: 'freestanding-emulator', source_commit: commit, image_sha256: image,
    host_id: host, boot_id: boot, build_id: 'build', image_id: 'image', profile_id: 'profile',
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
      face_id: 'face', face_revision: 1, host_id: host, boot_id: boot, offer_generation: 1,
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
        host_id: host, boot_id: boot, offer_generation: 1, mask_plan_id: show.plan_id,
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
      guest_boot_record: { boot_id: boot, host_id: host, build_id: 'build', image_binding: 'image', profile_id: 'profile' },
      frame: { checkpoint: names[i] },
      native_receipts: { latest_acknowledged_face: { face_id: 'face', face_revision: 1, basis: structuredClone(face.basis), show: structuredClone(receipt) } },
    });
  }
  let serial = '';
  chunks.forEach((chunk, index) => {
    serial += chunk.join('');
    manifest.checkpoints[index].serial_byte_end ??= Buffer.byteLength(serial);
  });
  return { proof, manifest, serial: Buffer.from(serial), faces: native };
}
