#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
#[path = "common/parser_joint_flows.rs"]
mod parser_joint_flows;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/parser_planned_runtime.rs"]
mod planned;
#[allow(dead_code)]
#[path = "common/parser_retained_v2.rs"]
mod retained;
#[path = "common/parser_joint_v2_runtime.rs"]
mod runtime;
#[path = "common/scorer_model.rs"]
mod scorer_model;
use retained::Session;
#[test]
#[ignore = "actual pinned model partial VOC2 acquisition; explicit cold Source/Plan preparation"]
fn actual_private_session_protects_partial_vocative_through_final_revision() {
    let profile = conduit_language::parser_model_selection::pinned_v2_lexical_profile().unwrap();
    let mut previous: Option<PreparedLexicalTape> = None;
    let mut history = Vec::new();
    let mut lexical_history = Vec::new();
    let mut inputs = Vec::new();
    let mut rows = Vec::new();
    for (sequence, text, finality, stable, count) in [
        (0, "Hello, Travis ", LanguageTextFinality::Partial, 13, 3),
        (1, "Hello, Travis.", LanguageTextFinality::Final, 14, 4),
    ] {
        let prior = previous.as_ref().map(|old| {
            LanguageTextPriorRevision::new(
                old.tape().source().material().revision().clone(),
                *old.tape().source().sequence(),
            )
            .unwrap()
        });
        let source = LanguageTextRevision::new(
            finality,
            LanguageText::new(
                LanguageTextId::new("stream/v2/independent-hello-travis".into()).unwrap(),
                profile.language().clone(),
                LanguageTextRevisionId::new(format!("independent-vocative/{sequence}")).unwrap(),
                text.into(),
            )
            .unwrap(),
            prior,
            profile.provenance().clone(),
            sequence,
            Some(stable),
        )
        .unwrap();
        let next = prepare_lexical_tape(&source, &profile, previous.as_ref()).unwrap();
        assert_eq!(next.tape().tokens().len(), count);
        if let Some(old) = &previous {
            for ordinal in 0..3 {
                let old_token = &old.tape().tokens()[ordinal];
                let new_token = &next.tape().tokens()[ordinal];
                assert_eq!(
                    new_token.prior_occurrence(),
                    &Some(old_token.identity().clone())
                );
                assert_eq!(new_token.span().start(), old_token.span().start());
                assert_eq!(new_token.span().end(), old_token.span().end());
                assert_eq!(new_token.span().basis(), old_token.span().basis());
                assert_eq!(
                    new_token.span().text_identity(),
                    old_token.span().text_identity()
                );
                assert_eq!(
                    new_token.span().text_revision(),
                    next.tape().source().material().revision()
                );
                assert_eq!(new_token.surface(), old_token.surface());
                assert_eq!(new_token.candidates(), old_token.candidates());
            }
        }
        history.push(source.into_structured().unwrap().canonical_bytes().unwrap());
        lexical_history.push(
            next.tape()
                .clone()
                .into_structured()
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
        inputs.push(Ok(LanguageParserJointLexical::new(
            next.tape().clone(),
            count as u64,
        )
        .unwrap()));
        // Reviewed targets are evaluation only. Native lexical alternatives and
        // actual planned inference determine every decoded choice and arc.
        rows.push(joint::Sentence {
            id: format!("independent-vocative/{sequence}"),
            forms: ["Hello", ",", "Travis", "."][..count]
                .iter()
                .map(|s| (*s).into())
                .collect(),
            pos: [6, 12, 11, 12][..count].to_vec(),
            heads: [4, 2, 0, 0][..count].to_vec(),
            relations: ["root", "punct", "vocative", "punct"][..count]
                .iter()
                .map(|s| (*s).into())
                .collect(),
            text: Some(text.into()),
        });
        previous = Some(next);
    }
    let mut session = Session::independently_protected(&profile);
    runtime::evaluate_observed(Some("independent-vocative"), rows, inputs, &mut session);
    let origins = session.protected_origins();
    assert!(origins.len() <= 4);
    let protected = origins
        .iter()
        .find(|origin| *origin.admission().fact().query().dependent() == 2)
        .expect("actual Partial VOC2 must enter private Source protection");
    assert!(matches!(
        protected
            .admission()
            .fact()
            .query()
            .beam()
            .lexical()
            .tape()
            .source()
            .finality(),
        LanguageTextFinality::Partial
    ));
    assert_eq!(*protected.admission().head(), 0);
    assert_eq!(
        *protected
            .admission()
            .fact()
            .query()
            .beam()
            .candidate0()
            .parser()
            .state()
            .committed(),
        0
    );
    let events = std::fs::read_to_string(
        std::env::var("CONDUIT_PARSER_STREAM_EVENTS_OUTPUT")
            .expect("actual private session requires retained event output"),
    )
    .unwrap();
    let events: Vec<serde_json::Value> = events
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let final_snapshot = events
        .iter()
        .rev()
        .find(|event| event["event"] == "snapshot")
        .unwrap();
    let custody = &final_snapshot["independent_protection"];
    let read_native = |value: &serde_json::Value| {
        let bytes: Vec<u8> = serde_json::from_value(value.clone()).unwrap();
        conduit_core::StructuredInfoValue::from_canonical_bytes(&bytes).unwrap()
    };
    let current = LanguageParserProtectedSetProposal::from_structured(read_native(
        &custody["current_set_bytes"],
    ))
    .unwrap();
    let hop = custody["rebase_receipts"]
        .as_array()
        .unwrap()
        .last()
        .expect("actual later revision must execute protected rebase");
    let context =
        LanguageParserProtectedSetRebaseContext::from_structured(read_native(&hop["input_bytes"]))
            .unwrap();
    let output =
        LanguageParserProtectedSetProposal::from_structured(read_native(&hop["output_bytes"]))
            .unwrap();
    assert!(current.active()[2]);
    assert_eq!(current.edge2(), output.edge2());
    assert_eq!(
        current.edge2().origin_basis(),
        protected.admission().fact().query().beam().basis()
    );
    assert_eq!(current.edge2().current_basis(), context.rebase().basis());
    assert_eq!(
        current.edge2().dependent_occurrence(),
        context.rebase().next().tape().tokens()[2].identity()
    );
    assert_eq!(
        current.edge2().head_occurrence(),
        context.rebase().next().tape().tokens()[0].identity()
    );
    assert_eq!(
        *context.rebase().next().tape().source().finality(),
        LanguageTextFinality::Final
    );
    assert_ne!(
        current.edge2().origin_basis().source_revision(),
        current.edge2().current_basis().source_revision()
    );
    let facts = session.retained_facts();
    let receipt = serde_json::json!({
        "schema":"language/parser-v2-private-protected-vocative-revision@1",
        "scope":"actual retained pinned-model private Session independent protection through Source revision; no played commitment",
        "final_independent_protection":custody,
        "native_origin_admission_bytes":session.protected_origins().iter().map(|origin| origin.clone().into_structured().unwrap().canonical_bytes().unwrap()).collect::<Vec<_>>(),
        "source_revision_history_bytes":history,
        "lexical_tape_history_bytes":lexical_history,
        "native_stable_fact_bytes":facts.iter().map(|f| f.clone().into_structured().unwrap().canonical_bytes().unwrap()).collect::<Vec<_>>(),
        "model_content_identity":serde_json::from_str::<serde_json::Value>(joint::MANIFEST).unwrap()["model_content_identity"],
        "model_signature_identity":joint::hex(conduit_language::parser_model_selection::pinned_v2_model_signature().unwrap().semantic_digest().unwrap()),
        "played_commitment":false,
    });
    if let Ok(path) = std::env::var("CONDUIT_PARSER_VOCATIVE_ACQUISITION_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    }
    let partial = facts.iter().find(|fact| {
        *fact.query().dependent() == 2
            && matches!(fact.query().beam().lexical().tape().source().finality(), LanguageTextFinality::Partial)
    }).expect("first actual partial decode did not admit stable VOC2; preserve predictions and investigate");
    let state = partial.query().beam().candidate0().parser().state();
    assert_eq!(state.heads()[2], 0);
    assert!(matches!(
        state.relation2().base(),
        LanguageUniversalDependencyRelation::Vocative
    ));
    assert_eq!(*state.committed(), 0);
    assert_eq!(
        partial.query().beam().lexical().tape().tokens()[2].surface(),
        "Travis"
    );
    assert!(!facts.iter().any(|fact| *fact.query().dependent() == 0
        && matches!(
            fact.query().beam().lexical().tape().source().finality(),
            LanguageTextFinality::Partial
        )));
}
