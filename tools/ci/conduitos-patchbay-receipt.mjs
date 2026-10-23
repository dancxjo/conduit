// Validate only the retained semantic evidence described by the native gallery.
// Native admission and execution remain owned by the producer and its QEMU proof.
export function validateNativePatchbayReceipt(proof) {
  const require = condition => {
    if (!condition) throw new Error('ConduitOS Patchbay journey evidence differs from its documentary claims');
  };
  const text = value => typeof value === 'string' && value.length > 0;
  const accepted = (receipt, intent) => receipt?.requested_intent === intent
    && receipt?.disposition?.Accepted != null && receipt.accepted_effects > 0
    && receipt.before?.body_id === proof.body_id && receipt.after?.body_id === proof.body_id;
  const actions = proof.patchbay_actions;
  const selection = actions?.cross_plot_selection;
  require(actions?.cross_plot_selection_enacted === true && selection?.same_play === true);
  for (const name of ['initial', 'other', 'returned']) {
    require(Number.isSafeInteger(selection[name]?.actor_revision));
    for (const key of ['source_document_id', 'checked_plot_id', 'expanded_plot_id']) {
      require(text(selection[name]?.[key]));
    }
  }
  for (const key of ['source_document_id', 'checked_plot_id', 'expanded_plot_id']) {
    require(selection.initial[key] === selection.returned[key]);
  }
  require(selection.other.checked_plot_id !== selection.initial.checked_plot_id
    && selection.initial.actor_revision < selection.other.actor_revision
    && selection.other.actor_revision < selection.returned.actor_revision);
  const owner = selection.initial.actual_face?.face?.basis;
  require(owner?.body_id === proof.body_id && text(owner?.plan_id) && text(owner?.active_play_id));
  for (const name of ['other', 'returned']) {
    const basis = selection[name].actual_face?.face?.basis;
    require(basis?.body_id === proof.body_id && basis?.plan_id === owner.plan_id
      && basis?.active_play_id === owner.active_play_id);
  }
  for (const name of ['inward', 'outward']) {
    const receipt = selection[name];
    require(accepted(receipt, 'patchbay/select-plot@1')
      && receipt.before?.active_play_id === owner.active_play_id
      && receipt.after?.active_play_id === owner.active_play_id
      && receipt.before?.plan_id === owner.plan_id && receipt.after?.plan_id === owner.plan_id);
  }
  const mask = actions.mask_request;
  require(actions.edit_outcome === 'native-editor-existing-draft'
    && actions.successful_edit_handoff_enacted === false
    && accepted(mask, 'patchbay/change-masks@1')
    && text(mask.before?.active_play_id) && text(mask.after?.active_play_id)
    && mask.before.active_play_id !== mask.after.active_play_id);

  const editor = proof.fresh_source_editor;
  require(editor && Array.isArray(editor.lifecycle) && editor.lifecycle.length === 4);
  for (const [index, intent] of ['lull', 'wake', 'plan', 'play'].entries()) {
    require(accepted(editor.lifecycle[index], `conduit.intent/${intent}@1`));
  }
  const fresh = editor.fresh;
  require(fresh?.body_id === proof.body_id && text(fresh?.plan_id) && text(fresh?.active_play_id)
    && editor.previous_live?.body_id === proof.body_id
    && text(editor.previous_live?.plan_id) && text(editor.previous_live?.active_play_id)
    && fresh.plan_id !== editor.previous_live?.plan_id
    && fresh.active_play_id !== editor.previous_live?.active_play_id
    && accepted(editor.typed_request, 'patchbay/edit-subject@1'));
  const handoff = editor.source_edit_receipt;
  const source = editor.source_correspondence;
  require(handoff?.body_plan_id === fresh.plan_id && handoff?.active_play_id === fresh.active_play_id
    && text(source?.source) && text(source?.source_document_id) && text(source?.checked_plot_id)
    && handoff?.source?.source_document_id === source.source_document_id
    && handoff?.source?.checked_plot_id === source.checked_plot_id
    && editor.selection_enacted === false && handoff.selection_enacted === false
    && editor.save_reopen_enacted === false);
  for (const name of ['loaded', 'suffix', 'restored']) {
    const state = editor[name];
    require(state?.body_id === proof.body_id && state?.plan_id === fresh.plan_id
      && state?.active_play_id === fresh.active_play_id && state?.result_omitted_bytes === 0);
  }
  require(editor.loaded.result === source.source && editor.suffix.result === `${source.source}x`
    && editor.restored.result === source.source);
  const face = editor.document_face?.face;
  require(Array.isArray(face?.subjects) && Array.isArray(face?.text)
    && face.subjects.some(subject => subject.role === 'Document'
      && face.text.some(item => item.subject === subject.identity && item.text === source.source)));
}
