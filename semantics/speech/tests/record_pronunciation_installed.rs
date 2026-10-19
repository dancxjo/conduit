#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
//! Supplied admitted dependency graphs exercise the actual consumer. This does
//! not claim learned parsing of these sentences or general pronunciation.
#[path = "common/record_pronunciation/carrier.rs"]
mod carrier;
#[path = "common/committed_carrier/commitment.rs"]
mod commitment;
#[path = "common/record_pronunciation/installed.rs"]
mod installed;
#[path = "common/record_pronunciation/inventory.rs"]
mod inventory;
#[path = "common/record_pronunciation/lexical.rs"]
mod lexical;
#[path = "common/record_pronunciation/refusals.rs"]
mod refusals;
use conduit_core::revision::*;
use conduit_language::{
    analysis_session::AnalysisSession,
    dependency_commitment::PreparedCommittedDependency,
    pronunciation_selection::prepare_pronunciation_selection,
    revision::{DependencyDelta, DependencyRevisions, TokenFrontier},
    *,
};
use conduit_speech::{
    complete_carrier::{CompleteCarrierAcoustics, PreparedCompleteSpeechCarrier},
    record_pronunciation::*,
    record_pronunciation_commitment::*,
};
fn text(s: &str) -> RevisionText<'_> {
    RevisionText::new(s).unwrap()
}
fn request(
    selection: &conduit_language::pronunciation_selection::PreparedPronunciationSelection,
) -> LanguageParserBeamCommitRequest {
    let q = selection.request();
    let basis = LanguageParserBasis::new(
        q.analysis().clone(),
        q.source().material().revision().clone(),
        q.source().material().identity().clone(),
    )
    .unwrap();
    let rel = |base| {
        LanguageParserRelation::new(base, LanguageParserSubtype::new("".into()).unwrap()).unwrap()
    };
    // Explicit finite supplied graph: token0 -> record, record -> root.
    // Native contracts admit this graph; actual commitment Source is replayed.
    let state = LanguageParserState::new(
        basis.clone(),
        0,
        3,
        [1, 4, 5, 5, 4],
        rel(*q.basis().relation().base()),
        rel(LanguageUniversalDependencyRelation::Root),
        rel(LanguageUniversalDependencyRelation::Dep),
        rel(LanguageUniversalDependencyRelation::Dep),
        [4, 0, 1, 4, 4],
        2,
        2,
    )
    .unwrap();
    let candidate =
        |active, id| LanguageParserHypothesis::new(active, id, 0, state.clone()).unwrap();
    LanguageParserBeamCommitRequest::new(
        LanguageParserAdmittedBeam::new(
            basis.clone(),
            candidate(true, 1),
            candidate(false, 2),
            candidate(false, 3),
            candidate(false, 4),
        )
        .unwrap(),
        false,
        LanguageParserRawFrontier::new(basis, 0, 2).unwrap(),
        false,
        2,
    )
    .unwrap()
}

#[test]
fn supplied_committed_record_pair_executes_installed_compact_formant_and_changes_record_pcm() {
    let owned = inventory::reviewed();
    let admitted = conduit_speech::ipa_inventory::PreparedIpaInventory::prepare(
        &owned.inventory,
        &owned.profile,
        owned.profile.variety(),
        owned.profile.revision(),
        &owned.phones,
        &owned.phonemes,
    )
    .unwrap();
    let profile = PreparedRecordPronunciationProfile::new().unwrap();
    let mut record_pcm = Vec::new();
    for (sentence, relation, prefix_frames) in [
        ("a record", LanguageUniversalDependencyRelation::Det, 800),
        (
            "we record",
            LanguageUniversalDependencyRelation::Nsubj,
            1600,
        ),
    ] {
        let lexical = lexical::lexical(sentence);
        let arc = lexical::arc(&lexical, relation);
        let selected = prepare_pronunciation_selection(
            &lexical,
            1,
            &lexical::analysis(),
            &arc,
            &lexical::profile(),
        )
        .unwrap();
        let request = request(&selected);
        let result = commitment::execute(&request);
        let dependency =
            PreparedCommittedDependency::prepare(&lexical, &arc, &request, &result).unwrap();
        let domain = DependencyRevisions {
            source_text: lexical.tape().source().material(),
            tokens: 2,
        };
        let context = RevisionContext {
            stream: text("record-analysis"),
            subject: text("record-pair"),
            epoch: text("1"),
            producer: text("supplied-graph-fixture"),
            policy: text("record-consumer@1"),
        };
        let reference = |sequence, label| RevisionReference {
            context,
            sequence,
            event: text(label),
        };
        let evidence = [RevisionEvidence {
            source: text("supplied-graph-fixture"),
            generation: text("1"),
        }];
        let delta = DependencyDelta::Assert(arc.clone());
        let proposed = RevisionEvent::new(
            &domain,
            reference(1, "proposed"),
            &evidence,
            RevisionChange::Proposed { delta: &delta },
        )
        .unwrap();
        let stable = RevisionEvent::new(
            &domain,
            reference(2, "stable"),
            &evidence,
            RevisionChange::Stable {
                revision: proposed.reference(),
                through: TokenFrontier(2),
            },
        )
        .unwrap();
        let commit = RevisionEvent::new(
            &domain,
            reference(3, "commit"),
            &evidence,
            RevisionChange::Committed {
                revision: proposed.reference(),
                through: TokenFrontier(2),
            },
        )
        .unwrap();
        let mut session = AnalysisSession::new(
            &domain,
            context,
            RevisionLimits {
                history_events: 8,
                revisable_units: 4,
            },
        )
        .unwrap();
        session.append(&proposed, None, false, false).unwrap();
        session.append(&stable, None, false, false).unwrap();
        session
            .append(&commit, Some(&dependency), false, false)
            .unwrap();
        let current = session.current(proposed.reference(), &dependency).unwrap();
        refusals::check(&profile, &admitted, &lexical, &current);
        let pronunciation =
            CommittedRecordPronunciation::prepare(&profile, &selected, &admitted, &current)
                .unwrap();
        let f = carrier::build(pronunciation.supplied());
        let inventory = f.inventory().unwrap();
        let correspondence = f.correspondence(&inventory).unwrap();
        let structure = conduit_speech::syllable_structure::PreparedSyllableStructure::prepare(
            &f.intent,
            &f.phones,
            f.source.material(),
            &f.syllables,
            &f.positioned_boundaries,
        )
        .unwrap();
        let complete = PreparedCompleteSpeechCarrier::prepare(
            &correspondence,
            &structure,
            &lexical,
            &f.word_phonemes,
            &request,
            &result,
            CompleteCarrierAcoustics::not_supplied(),
        )
        .unwrap();
        let pcm = installed::execute(
            &pronunciation,
            &complete,
            &f.formant,
            &f.boundaries,
            inventory::provenance(),
        );
        assert_eq!(pcm.len(), (prefix_frames + 4800) * 2);
        // Remove the different prefix using exact declared 8k duration. Compare
        // only the complete record span, including its differing stress/vowel.
        record_pcm.push(pcm[prefix_frames * 2..].to_vec());
    }
    assert_eq!(record_pcm[0].len(), 9600);
    assert_eq!(record_pcm[1].len(), 9600);
    assert_ne!(record_pcm[0], record_pcm[1]);
}
