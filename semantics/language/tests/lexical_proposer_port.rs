include!("lexical_proposer_resource.rs");
use conduit_language::lexical_proposer_port::*;
mod fixture {
    use super::*;
    pub fn provenance() -> LinguisticDerivationProvenance {
        LinguisticDerivationProvenance::deterministic_rule(
            "reviewed-proposal-policy".into(),
            "1".into(),
        )
        .unwrap()
    }
    pub fn definition() -> LanguageLexicalProposerDefinition {
        LanguageLexicalProposerDefinition::new(
            "dictionary-full-resource".into(),
            "proposer".into(),
            LanguageId::new("language/en".into()).unwrap(),
            provenance(),
            LanguageLexicalUnknownPolicy::new(
                BoundedSequence::try_from_iter([
                    LanguageLexicalPos::Noun,
                    LanguageLexicalPos::Verb,
                ])
                .unwrap(),
                LanguageLexicalUnknownCommitPolicy::StableInputConsensus,
                "unknown-policy".into(),
                provenance(),
            )
            .unwrap(),
        )
        .unwrap()
    }
    pub fn token(lemma: &str) -> LanguageLexicalToken {
        token_surface(lemma, "unlisted")
    }
    pub fn token_surface(lemma: &str, surface: &str) -> LanguageLexicalToken {
        LanguageLexicalToken::new(
            BoundedSequence::try_from_iter(
                [LanguageLexicalPos::Noun, LanguageLexicalPos::Verb]
                    .into_iter()
                    .map(|pos| {
                        LanguageLexicalCandidate::new(lemma.into(), BoundedSequence::new(), pos)
                            .unwrap()
                    }),
            )
            .unwrap(),
            LinguisticTokenCategory::Word,
            LanguageLexicalCompleteness::Complete,
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
    }
    pub fn origin(id: &str) -> LanguageLexicalProposalOrigin {
        LanguageLexicalProposalOrigin::unknown(id.into(), provenance()).unwrap()
    }
}

fn port_limits() -> LexicalProposerPortLimits {
    LexicalProposerPortLimits {
        maximum_frame_bytes: 262144,
        maximum_retained_bytes: 300_000_000,
        maximum_preparation_peak_bytes: 2_000_000_000,
        maximum_admission_peak_bytes: 2_000_000_000,
        maximum_response_requested_bytes: 1_000_000_000,
        native: native(),
    }
}
fn dictionary() -> resource::PreparedLexicalDictionary {
    resource::PreparedLexicalDictionary::prepare(
        vec![shard("record", "language/en")],
        "language/en",
        limits(),
    )
    .unwrap()
}
fn bound_definition(d: &resource::PreparedLexicalDictionary) -> LanguageLexicalProposerDefinition {
    let hex = d
        .identity()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    LanguageLexicalProposerDefinition::new(
        hex,
        "proposer".into(),
        LanguageId::new("language/en".into()).unwrap(),
        fixture::provenance(),
        fixture::definition().unknown().clone(),
    )
    .unwrap()
}
#[test]
fn original_source_query_resource_custody_and_capacity() {
    let d = dictionary();
    let definition = bound_definition(&d);
    let canonical_definition = definition.clone().encode().unwrap();
    let query = LanguageLexicalProposalQuery::new(
        definition.clone(),
        fixture::origin("unknown-policy"),
        None,
        fixture::token("<unknown>"),
    )
    .unwrap();
    let canonical = query.clone().encode().unwrap();
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TRACK.store(true, Ordering::SeqCst);
    let mut port =
        PreparedLexicalProposerPort::prepare(d, &canonical_definition, port_limits()).unwrap();
    TRACK.store(false, Ordering::SeqCst);
    let preparation_observed = (PEAK.load(Ordering::SeqCst) - baseline).max(0) as usize;
    let r = port.storage_receipt();
    eprintln!("port_receipt={r:?}");
    assert!(r.preparation_peak_heap_bytes_bound >= preparation_observed);
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TRACK.store(true, Ordering::SeqCst);
    let response = port.admit_query(&canonical).unwrap();
    TRACK.store(false, Ordering::SeqCst);
    let admission_observed = (PEAK.load(Ordering::SeqCst) - baseline).max(0) as usize;
    let response_observed = (LIVE.load(Ordering::SeqCst) - baseline).max(0) as usize;
    assert!(r.admission_peak_heap_bytes_bound - r.retained_heap_bytes_bound >= admission_observed);
    assert!(response.requested_bytes_bound() >= response_observed);
    eprintln!("port preparation_peak_observed={preparation_observed} admission_peak_observed={admission_observed} response_retained_observed={response_observed}");
    let known_as_unknown = LanguageLexicalProposalQuery::new(
        definition.clone(),
        fixture::origin("unknown-policy"),
        None,
        fixture::token_surface("<unknown>", "record"),
    )
    .unwrap()
    .encode()
    .unwrap();
    assert!(matches!(
        port.admit_query(&known_as_unknown),
        Err(LexicalProposerPortRefusal::Resource)
    ));
    assert_eq!(response.canonical_query(), canonical);
    assert_eq!(response.proposal().query(), &query);
    assert_eq!(
        LanguageLexicalTokenProposal::decode(response.canonical_output()).unwrap(),
        *response.proposal()
    );
    assert!(matches!(
        response.proposal().query().origin(),
        LanguageLexicalProposalOrigin::Unknown(_)
    ));
    assert!(matches!(
        port.admit_query(b"foreign"),
        Err(LexicalProposerPortRefusal::Frame)
    ));
    let other = LanguageLexicalProposalQuery::new(
        fixture::definition(),
        fixture::origin("unknown-policy"),
        None,
        fixture::token("<unknown>"),
    )
    .unwrap()
    .encode()
    .unwrap();
    assert!(matches!(
        port.admit_query(&other),
        Err(LexicalProposerPortRefusal::Definition)
    ));
    for field in 0..4 {
        let mut l = port_limits();
        match field {
            0 => l.maximum_retained_bytes = r.retained_heap_bytes_bound - 1,
            1 => l.maximum_preparation_peak_bytes = r.preparation_peak_heap_bytes_bound - 1,
            2 => l.maximum_admission_peak_bytes = r.admission_peak_heap_bytes_bound - 1,
            _ => l.maximum_response_requested_bytes = r.response_requested_bytes_bound - 1,
        }
        assert!(
            PreparedLexicalProposerPort::prepare(dictionary(), &canonical_definition, l).is_err()
        );
    }
}
#[test]
fn reviewed_resource_exact_entry_and_shard_custody() {
    let mut d = dictionary();
    let definition = bound_definition(&d);
    let lookup = d.lookup("record").unwrap().unwrap();
    let hex = |digest: [u8; 32]| {
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let token = LanguageLexicalToken::new(
        lookup.entry().candidates().clone(),
        LinguisticTokenCategory::Word,
        LanguageLexicalCompleteness::Complete,
        LinguisticTokenIdentity::new(
            0,
            LanguageTextId::new("original".into()).unwrap(),
            LanguageTextRevisionId::new("original-r0".into()).unwrap(),
        )
        .unwrap(),
        None,
        TextSpan::new(
            LinguisticOffsetBasis::UnicodeScalar,
            6,
            0,
            LanguageTextId::new("original".into()).unwrap(),
            LanguageTextRevisionId::new("original-r0".into()).unwrap(),
        )
        .unwrap(),
        "record".into(),
    )
    .unwrap();
    let origin = LanguageLexicalProposalOrigin::reviewed(
        hex(lookup.dictionary_identity()),
        u64::from(lookup.ordinal()),
        u64::from(lookup.shard()),
        hex(lookup.shard_identity()),
    )
    .unwrap();
    let query = LanguageLexicalProposalQuery::new(
        definition.clone(),
        origin,
        Some(lookup.entry().clone()),
        token.clone(),
    )
    .unwrap();
    let forged = LanguageLexicalProposalQuery::new(
        definition.clone(),
        LanguageLexicalProposalOrigin::reviewed(
            hex(lookup.dictionary_identity()),
            0,
            0,
            "foreign-shard".into(),
        )
        .unwrap(),
        Some(lookup.entry().clone()),
        token,
    )
    .unwrap();
    let mut port =
        PreparedLexicalProposerPort::prepare(d, &definition.encode().unwrap(), port_limits())
            .unwrap();
    let response = port.admit_query(&query.clone().encode().unwrap()).unwrap();
    assert_eq!(response.proposal().query(), &query);
    assert!(matches!(
        port.admit_query(&forged.encode().unwrap()),
        Err(LexicalProposerPortRefusal::Resource)
    ));
}
