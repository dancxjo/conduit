// Structural publication evidence only; this fixture does not enact native work.
export function patchbayFixture(body) {
  const state = (plan = 'plan/fresh', play = 'play/fresh') => ({
    body_id: body, plan_id: plan, active_play_id: play,
  });
  const action = (intent, before = state(), after = state()) => ({
    requested_intent: intent, before, after, accepted_effects: 1, disposition: { Accepted: {} },
  });
  const source = 'plot retained_source {}\n';
  const correspondence = (actor_revision, name) => ({ actor_revision,
    source_document_id: `source/${name}`, checked_plot_id: `checked/${name}`,
    expanded_plot_id: `expanded/${name}`, actual_face: { face: { basis: state() } } });
  const document = result => ({ ...state(), result, result_omitted_bytes: 0 });
  return {
    patchbay_actions: {
      cross_plot_selection_enacted: true,
      cross_plot_selection: {
        initial: correspondence(1, 'original'), other: correspondence(2, 'other'),
        returned: correspondence(3, 'original'), same_play: true,
        inward: action('patchbay/select-plot@1'), outward: action('patchbay/select-plot@1'),
      },
      edit_outcome: 'native-editor-existing-draft', successful_edit_handoff_enacted: false,
      mask_request: action('patchbay/change-masks@1', state('plan/old', 'play/old'), state()),
    },
    fresh_source_editor: {
      previous_live: state('plan/old', 'play/old'), fresh: state(),
      lifecycle: ['lull', 'wake', 'plan', 'play'].map(intent => action(`conduit.intent/${intent}@1`)),
      typed_request: action('patchbay/edit-subject@1'),
      source_correspondence: { source, source_document_id: 'source/editor', checked_plot_id: 'checked/editor' },
      source_edit_receipt: { body_plan_id: 'plan/fresh', active_play_id: 'play/fresh',
        source: { source_document_id: 'source/editor', checked_plot_id: 'checked/editor' }, selection_enacted: false },
      loaded: document(source), suffix: document(`${source}x`), restored: document(source),
      document_face: { face: { subjects: [{ role: 'Document', identity: 'document' }],
        text: [{ subject: 'document', text: source }] } },
      selection_enacted: false, save_reopen_enacted: false,
    },
  };
}
