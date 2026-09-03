use conduit_tongues::{
    check_research_forms, run_research, shared_latent_signature, shared_relation_signature,
    Pb2007Slice, RESEARCH_SEED, TRAINING_FORM_SOURCE,
};

#[test]
fn exact_corpus_is_split_and_label_free_at_training_boundary() {
    let corpus = Pb2007Slice::load().expect("exact corpus slice");
    let training = corpus.training_utterances();
    assert_eq!(training.len(), 12);
    assert_eq!(
        training
            .iter()
            .filter(|value| value.split == "train")
            .count(),
        8
    );
    assert_eq!(
        training
            .iter()
            .filter(|value| value.split == "validation")
            .count(),
        2
    );
    assert_eq!(
        training
            .iter()
            .filter(|value| value.split == "test")
            .count(),
        2
    );
    assert_eq!(corpus.source.archive_sha256.len(), 64);
}

#[test]
fn portable_forms_check_and_training_form_has_no_labels_or_realization_facts() {
    let forms = check_research_forms().expect("research Forms check and expand");
    assert_eq!(forms[0].gears.len(), 2);
    assert_eq!(forms[1].gears.len(), 4);
    for forbidden in ["phone", "syllable", "IPA", "host", "device", "runtime"] {
        assert!(
            !TRAINING_FORM_SOURCE.contains(forbidden),
            "found {forbidden}"
        );
    }
}

#[test]
fn bounded_run_is_reproducible_bidirectional_and_honest() {
    let first = run_research().expect("research experiment runs");
    let second = run_research().expect("same research experiment reruns");
    assert_eq!(
        first.training.checkpoint_identity,
        second.training.checkpoint_identity
    );
    assert_eq!(first.training.seed, RESEARCH_SEED);
    assert!(!first.training.labels_visible_to_trainer);
    assert_ne!(
        first.training.checkpoint_identity,
        first.alternate_checkpoint_identity
    );
    assert_eq!(first.held_out.len(), 2);
    assert_eq!(first.bidirectional_query.audio_to_latent.len(), 2);
    assert_eq!(first.bidirectional_query.articulation_to_latent.len(), 2);
    assert_eq!(first.bidirectional_query.generated_audio.len(), 4);
    assert_eq!(
        first.bidirectional_query.inferred_articulation.mean.len(),
        6
    );
    assert_eq!(
        first
            .bidirectional_query
            .inferred_articulation
            .alternatives
            .len(),
        2
    );
    assert_eq!(
        first.bidirectional_query.inferred_articulation.disposition,
        "inferred-not-observed"
    );
    assert!(first.post_freeze_probe.checkpoint_frozen_before_labels);
    assert!(!first.post_freeze_probe.unsupported_claims.is_empty());
    assert!(first
        .limitations
        .iter()
        .any(|value| value.contains("one PB2007 speaker")));
    let signature = shared_latent_signature();
    signature.validate().expect("finite callable signature");
    shared_relation_signature(&signature)
        .validate()
        .expect("both directions are finite #2142 relation patterns");
}
