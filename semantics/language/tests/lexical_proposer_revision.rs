include!("lexical_proposer_resource.rs");
use conduit_language::lexical_proposer_port::{self, token_producer::*, *};
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule("new-proposer-policy".into(), "1".into())
        .unwrap()
}
fn port() -> PreparedLexicalProposerPort {
    port_for("record", "unknown-policy")
}
fn port_for(surface: &str, policy_identity: &str) -> PreparedLexicalProposerPort {
    let dictionary = resource::PreparedLexicalDictionary::prepare(
        vec![shard(surface, "language/en")],
        "language/en",
        limits(),
    )
    .unwrap();
    let hex = dictionary
        .identity()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let definition = LanguageLexicalProposerDefinition::new(
        hex,
        "new-proposer".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
        LanguageLexicalUnknownPolicy::new(
            BoundedSequence::try_from_iter([LanguageLexicalPos::Noun, LanguageLexicalPos::Verb])
                .unwrap(),
            LanguageLexicalUnknownCommitPolicy::StableInputConsensus,
            policy_identity.into(),
            provenance(),
        )
        .unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap();
    PreparedLexicalProposerPort::prepare(
        dictionary,
        &definition,
        LexicalProposerPortLimits {
            maximum_frame_bytes: 262144,
            maximum_retained_bytes: 300_000_000,
            maximum_preparation_peak_bytes: 2_000_000_000,
            maximum_admission_peak_bytes: 2_000_000_000,
            maximum_response_requested_bytes: 1_000_000_000,
            native: native(),
        },
    )
    .unwrap()
}
fn producer_limits() -> TokenProducerLimits {
    TokenProducerLimits {
        maximum_retained_bytes: 300_000_000,
        maximum_preparation_peak_bytes: 2_000_000_000,
        maximum_admission_peak_bytes: 2_000_000_000,
        maximum_response_retained_bytes: 10_000_000,
    }
}
fn token(surface: &str, partial: bool) -> Vec<u8> {
    LanguageLexicalToken::new(
        BoundedSequence::new(),
        LinguisticTokenCategory::Word,
        if partial {
            LanguageLexicalCompleteness::TrailingPartial
        } else {
            LanguageLexicalCompleteness::Complete
        },
        LinguisticTokenIdentity::new(
            0,
            LanguageTextId::new("original".into()).unwrap(),
            LanguageTextRevisionId::new("original-r0".into()).unwrap(),
        )
        .unwrap(),
        None,
        TextSpan::new(
            LinguisticOffsetBasis::UnicodeScalar,
            surface.chars().count() as u64,
            0,
            LanguageTextId::new("original".into()).unwrap(),
            LanguageTextRevisionId::new("original-r0".into()).unwrap(),
        )
        .unwrap(),
        surface.into(),
    )
    .unwrap()
    .encode()
    .unwrap()
}
#[test]
fn original_source_unknown_reviewed_and_partial_production() {
    let mut p = PreparedTokenProducer::prepare(port(), producer_limits()).unwrap();
    eprintln!("producer_receipt={:?}", p.storage_receipt());
    for (surface, partial) in [("glorp", false), ("record", false), ("glor", true)] {
        let original = token(surface, partial);
        let produced = p.propose(&original).unwrap();
        assert_eq!(produced.original_token(), original);
        let proposal =
            LanguageLexicalTokenProposal::decode(produced.proposal().canonical_output()).unwrap();
        let base = LanguageLexicalToken::decode(&original).unwrap();
        let observed = proposal.query().token();
        assert_eq!(observed.identity(), base.identity());
        assert_eq!(observed.span(), base.span());
        assert_eq!(observed.surface(), base.surface());
        assert_eq!(observed.completeness(), base.completeness());
        assert_eq!(observed.category(), base.category());
        assert_eq!(observed.prior_occurrence(), base.prior_occurrence());
        match proposal.query().origin() {
            LanguageLexicalProposalOrigin::Unknown(_) => {
                assert_eq!(surface, "glorp");
                assert_eq!(observed.candidates().len(), 2);
                assert!(observed
                    .candidates()
                    .iter()
                    .all(|c| c.lemma() == "<unknown>"));
            }
            LanguageLexicalProposalOrigin::Reviewed(_) => {
                assert_eq!(surface, "record");
                assert_eq!(observed.candidates()[0].lemma(), "record");
            }
            LanguageLexicalProposalOrigin::Incomplete => {
                assert!(partial);
                assert!(observed.candidates().is_empty());
            }
        }
    }
    assert!(p.propose(b"foreign").is_err());
}
#[test]
fn whole_preparation_and_response_allocation_bounds_and_one_under() {
    let source_port = port();
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TRACK.store(true, Ordering::SeqCst);
    let mut producer = PreparedTokenProducer::prepare(source_port, producer_limits()).unwrap();
    TRACK.store(false, Ordering::SeqCst);
    let preparation = (PEAK.load(Ordering::SeqCst) - baseline).max(0) as usize;
    let receipt = producer.storage_receipt();
    assert!(receipt.preparation_peak_heap_bytes_bound >= preparation);
    for surface in ["glorp", "record"] {
        let original = token(surface, false);
        let baseline = LIVE.load(Ordering::SeqCst);
        PEAK.store(baseline, Ordering::SeqCst);
        TRACK.store(true, Ordering::SeqCst);
        let response = producer.propose(&original).unwrap();
        TRACK.store(false, Ordering::SeqCst);
        let peak = (PEAK.load(Ordering::SeqCst) - baseline).max(0) as usize;
        let retained = (LIVE.load(Ordering::SeqCst) - baseline).max(0) as usize;
        assert!(
            receipt.admission_peak_heap_bytes_bound - receipt.retained_heap_bytes_bound >= peak
        );
        assert!(response.retained_requested_bytes_bound() >= retained);
        assert!(receipt.response_retained_bytes_bound >= response.retained_requested_bytes_bound());
        eprintln!("surface={surface} producer_preparation_peak_increment={preparation} admission_peak_increment={peak} response_retained={retained} response_charge={}",response.retained_requested_bytes_bound());
    }
    for ceiling in 0..4 {
        let mut limits = producer_limits();
        match ceiling {
            0 => limits.maximum_retained_bytes = receipt.retained_heap_bytes_bound - 1,
            1 => {
                limits.maximum_preparation_peak_bytes =
                    receipt.preparation_peak_heap_bytes_bound - 1
            }
            2 => limits.maximum_admission_peak_bytes = receipt.admission_peak_heap_bytes_bound - 1,
            _ => limits.maximum_response_retained_bytes = receipt.response_retained_bytes_bound - 1,
        }
        assert!(matches!(
            PreparedTokenProducer::prepare(port(), limits),
            Err(TokenProducerRefusal::Capacity)
        ));
    }
}
fn original_revision(
    sequence: u64,
    prior: Option<&LanguageTextRevision>,
    text: &str,
    stable: Option<u32>,
    partial: bool,
) -> LanguageTextRevision {
    LanguageTextRevision::new(
        if partial {
            LanguageTextFinality::Partial
        } else {
            LanguageTextFinality::Final
        },
        LanguageText::new(
            LanguageTextId::new("stream".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new(format!("r{sequence}")).unwrap(),
            text.into(),
        )
        .unwrap(),
        prior.map(|p| {
            LanguageTextPriorRevision::new(p.material().revision().clone(), *p.sequence()).unwrap()
        }),
        provenance(),
        sequence,
        stable,
    )
    .unwrap()
}
fn revision_limits() -> lexical_proposer_port::token_producer::revision::RevisionLimits {
    lexical_proposer_port::token_producer::revision::RevisionLimits {
        maximum_tokens: 128,
        maximum_retained_bytes: 1_000_000_000,
        maximum_preparation_peak_bytes: 2_000_000_000,
        maximum_admission_peak_bytes: 3_000_000_000,
        maximum_response_retained_bytes: 1_000_000_000,
    }
}
#[test]
fn complete_original_revision_successor_custody_and_atomic_refusal() {
    let producer = PreparedTokenProducer::prepare(port(), producer_limits()).unwrap();
    let mut owner =
        lexical_proposer_port::token_producer::revision::PreparedRevisionProducer::prepare(
            producer,
            revision_limits(),
        )
        .unwrap();
    println!("revision_receipt={:?}", owner.storage_receipt());
    let r0 = original_revision(0, None, "record glorp", Some(6), true);
    let first = owner
        .propose(&r0.clone().encode().unwrap(), None, 0)
        .unwrap();
    assert_eq!(first.tokens().len(), 2);
    let first_native =
        LanguageLexicalProposedTape::decode(first.canonical_proposed_tape()).unwrap();
    assert_eq!(
        first_native
            .tape()
            .tokens()
            .iter()
            .next()
            .unwrap()
            .candidates()
            .len(),
        1
    );
    assert!(first_native
        .tape()
        .tokens()
        .iter()
        .nth(1)
        .unwrap()
        .candidates()
        .is_empty());
    let r1 = original_revision(1, Some(&r0), "record glorp!", Some(12), false);
    let second = owner
        .propose(&r1.clone().encode().unwrap(), Some(&first), 0)
        .unwrap();
    let next = LanguageLexicalProposedTape::decode(second.canonical_proposed_tape()).unwrap();
    assert_eq!(next.tape().tokens().len(), 3);
    assert_eq!(
        next.tape()
            .tokens()
            .iter()
            .next()
            .unwrap()
            .prior_occurrence()
            .as_ref(),
        Some(
            first_native
                .tape()
                .tokens()
                .iter()
                .next()
                .unwrap()
                .identity()
        )
    );
    assert!(next
        .tape()
        .tokens()
        .iter()
        .nth(1)
        .unwrap()
        .prior_occurrence()
        .is_none());
    assert_eq!(
        next.tape()
            .tokens()
            .iter()
            .nth(1)
            .unwrap()
            .candidates()
            .len(),
        2
    );
    let before = first.canonical_proposed_tape().to_vec();
    let bad = original_revision(1, Some(&r0), "reword glorp", Some(6), false);
    assert!(owner
        .propose(&bad.encode().unwrap(), Some(&first), 0)
        .is_err());
    assert_eq!(first.canonical_proposed_tape(), before);
    assert!(owner
        .propose(&r1.clone().encode().unwrap(), None, 0)
        .is_err());
    assert!(owner.propose(&[0], Some(&first), 0).is_err());
}
#[test]
fn whole_revision_resource_peak_retained_and_one_under() {
    let producer = PreparedTokenProducer::prepare(port(), producer_limits()).unwrap();
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TRACK.store(true, Ordering::SeqCst);
    let mut owner =
        lexical_proposer_port::token_producer::revision::PreparedRevisionProducer::prepare(
            producer,
            revision_limits(),
        )
        .unwrap();
    TRACK.store(false, Ordering::SeqCst);
    let prep = (PEAK.load(Ordering::SeqCst) - baseline).max(0) as usize;
    let receipt = owner.storage_receipt();
    assert!(receipt.preparation_peak_bytes_bound >= prep);
    let r0 = original_revision(0, None, "record glorp!", Some(12), false)
        .encode()
        .unwrap();
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TRACK.store(true, Ordering::SeqCst);
    let first = owner.propose(&r0, None, 0).unwrap();
    TRACK.store(false, Ordering::SeqCst);
    let peak = (PEAK.load(Ordering::SeqCst) - baseline).max(0) as usize;
    let retained = (LIVE.load(Ordering::SeqCst) - baseline).max(0) as usize;
    assert!(receipt.admission_peak_bytes_bound - receipt.retained_bytes_bound >= peak);
    assert!(first.retained_bytes_bound() >= retained);
    assert!(receipt.response_retained_bytes_bound >= first.retained_bytes_bound());
    println!("revision_preparation_increment={prep} admission_increment={peak} retained={retained} charge={}",first.retained_bytes_bound());
    for which in 0..4 {
        let mut limits = revision_limits();
        match which {
            0 => limits.maximum_retained_bytes = receipt.retained_bytes_bound - 1,
            1 => limits.maximum_preparation_peak_bytes = receipt.preparation_peak_bytes_bound - 1,
            2 => limits.maximum_admission_peak_bytes = receipt.admission_peak_bytes_bound - 1,
            _ => limits.maximum_response_retained_bytes = receipt.response_retained_bytes_bound - 1,
        };
        let producer = PreparedTokenProducer::prepare(port(), producer_limits()).unwrap();
        assert!(matches!(
            lexical_proposer_port::token_producer::revision::PreparedRevisionProducer::prepare(
                producer, limits
            ),
            Err(TokenProducerRefusal::Capacity)
        ));
    }
}
#[test]
fn revision_scalar_limit_and_committed_frontier_refusals() {
    let producer = PreparedTokenProducer::prepare(port(), producer_limits()).unwrap();
    let mut limits = revision_limits();
    limits.maximum_tokens = 2;
    let mut owner =
        lexical_proposer_port::token_producer::revision::PreparedRevisionProducer::prepare(
            producer, limits,
        )
        .unwrap();
    let unicode = original_revision(0, None, "naïve 中文", Some(8), false)
        .encode()
        .unwrap();
    let accepted = owner.propose(&unicode, None, 0).unwrap();
    let tape = LanguageLexicalProposedTape::decode(accepted.canonical_proposed_tape()).unwrap();
    let token = tape.tape().tokens().iter().nth(1).unwrap();
    assert_eq!(*token.span().start(), 6);
    assert_eq!(*token.span().end(), 8);
    assert!(owner
        .propose(
            &original_revision(0, None, "a b c", None, false)
                .encode()
                .unwrap(),
            None,
            0
        )
        .is_err());
    assert!(owner
        .propose(
            &original_revision(0, None, "a", None, false)
                .encode()
                .unwrap(),
            None,
            1
        )
        .is_err());
    let empty = owner
        .propose(
            &original_revision(0, None, "", None, false)
                .encode()
                .unwrap(),
            None,
            0,
        )
        .unwrap();
    assert!(empty.tokens().is_empty());
}

#[cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
#[path = "common/parser_model_resource.rs"]
mod proposal_model_resource;
#[cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
#[test]
fn distinct_proposal_model_declaration_full_material_and_foreign_refusals() {
    use conduit_language::lexical_proposer_port::token_producer::model_definition::*;
    use conduit_language::parser_model_selection::*;
    use std::sync::Arc;
    const BYTES: &[u8] = include_bytes!("../training/ewt_joint_v2/ewt_joint.i16");
    let owner = lexical_proposer_port::token_producer::revision::PreparedRevisionProducer::prepare(
        PreparedTokenProducer::prepare(port(), producer_limits()).unwrap(),
        revision_limits(),
    )
    .unwrap();
    let model = proposal_model_resource::categorical(
        BYTES.to_vec(),
        pinned_v2_model_signature().unwrap(),
        1,
    );
    // Synthetic metadata admission only: no training, inference, Source execution,
    // legacy-profile migration or linguistic acceptance is claimed by this test.
    let source = ParserSourceModelContract {
        feature_contract: [1; 32],
        availability_contract: [2; 32],
        action_contract: [3; 32],
        joint_choice_contract: [4; 32],
        numeric_indices_contract: model.indices_type().semantic_digest().unwrap(),
        numeric_scores_contract: model.scores_type().semantic_digest().unwrap(),
    };
    let make = |dimensions, maximum, copy_bound| {
        ProposalModelDefinition::new(
            &owner,
            "synthetic/proposal-model@1".into(),
            "synthetic/test-features@1".into(),
            source.clone(),
            model.resource().artifact().clone(),
            model.resource().signature().clone(),
            dimensions,
            maximum,
            b"synthetic-training-manifest; not actual trained artifact"
                .to_vec()
                .into(),
            copy_bound,
        )
    };
    let definition = Arc::new(make(model.dimensions(), V2_MAXIMUM_SCORE, 262144).unwrap());
    let selected =
        PreparedProposalModelSelection::prepare(&owner, model.clone(), definition.clone(), &source)
            .unwrap();
    assert_eq!(
        selected.declaration().canonical_proposer_definition(),
        definition.canonical_proposer_definition()
    );
    assert_eq!(
        selected.declaration().artifact(),
        model.resource().artifact()
    );
    assert_eq!(
        selected.declaration().training_manifest(),
        b"synthetic-training-manifest; not actual trained artifact"
    );
    assert_eq!(
        selected.declaration().segmentation_abi(),
        ProposalSegmentationAbi::UnicodeScalarAlphanumericApostropheV1
    );
    assert_eq!(
        definition.proposal_source_material(),
        include_bytes!("../lexical_proposer.conduit")
    );
    assert_ne!(definition.proposal_source_identity(), [0; 32]);
    for foreign_port in [
        port_for("old", "unknown-policy"),
        port_for("record", "foreign-policy"),
    ] {
        let foreign_owner =
            lexical_proposer_port::token_producer::revision::PreparedRevisionProducer::prepare(
                PreparedTokenProducer::prepare(foreign_port, producer_limits()).unwrap(),
                revision_limits(),
            )
            .unwrap();
        assert_eq!(
            PreparedProposalModelSelection::prepare(
                &foreign_owner,
                model.clone(),
                definition.clone(),
                &source
            )
            .err(),
            Some(ProposalModelRefusal::Proposer)
        );
    }
    let mut foreign = source.clone();
    foreign.feature_contract[0] ^= 1;
    assert_eq!(
        PreparedProposalModelSelection::prepare(
            &owner,
            model.clone(),
            definition.clone(),
            &foreign
        )
        .err(),
        Some(ProposalModelRefusal::Source)
    );
    let dimensions = model.dimensions();
    let wrong = Arc::new(
        make(
            (dimensions.0 + 1, dimensions.1, dimensions.2),
            V2_MAXIMUM_SCORE,
            262144,
        )
        .unwrap(),
    );
    assert_eq!(
        PreparedProposalModelSelection::prepare(&owner, model.clone(), wrong, &source).err(),
        Some(ProposalModelRefusal::Dimensions)
    );
    let wrong = Arc::new(make(dimensions, 0, 262144).unwrap());
    assert_eq!(
        PreparedProposalModelSelection::prepare(&owner, model.clone(), wrong, &source).err(),
        Some(ProposalModelRefusal::Score)
    );
    let mut changed = BYTES.to_vec();
    changed[20] ^= 1;
    let other =
        proposal_model_resource::categorical(changed, pinned_v2_model_signature().unwrap(), 1);
    assert_eq!(
        PreparedProposalModelSelection::prepare(&owner, other, definition.clone(), &source).err(),
        Some(ProposalModelRefusal::Artifact)
    );
    assert_eq!(
        make(
            dimensions,
            V2_MAXIMUM_SCORE,
            definition.canonical_proposer_definition().len() - 1
        )
        .err(),
        Some(ProposalModelRefusal::Capacity)
    );
}
