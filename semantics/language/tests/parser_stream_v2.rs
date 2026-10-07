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
#[path = "common/parser_retained_v2.rs"]
mod retained;
#[path = "common/parser_joint_v2_runtime.rs"]
mod runtime;
#[path = "common/scorer_model.rs"]
mod scorer_model;
use retained::Session;
#[test]
#[ignore = "actual retained partial-source learned stream evidence; cold finite Plan preparation measured"]
fn later_complete_words_revise_provisional_parse_in_retained_flows() {
    let template = joint::Sentence {
        id: "stream-profile".into(),
        forms: vec!["Travis".into(), "Hello".into()],
        pos: vec![11, 6],
        heads: vec![1, 4],
        relations: vec!["vocative".into(), "root".into()],
        text: None,
    };
    let reference = joint::lexical(&template).unwrap();
    let profile = reference.tape().profile();
    let mut prepared: Option<PreparedLexicalTape> = None;
    let mut history = Vec::new();
    let mut rows = Vec::new();
    let mut inputs = Vec::new();
    let mut session = Session::new(profile);
    for (sequence, text, finality, stable) in [
        (0, "Tr", LanguageTextFinality::Partial, None),
        (1, "Travis ", LanguageTextFinality::Partial, Some(7)),
        (2, "Travis Hello ", LanguageTextFinality::Partial, Some(13)),
        (3, "Travis Hello ", LanguageTextFinality::Final, Some(13)),
    ] {
        let prior = prepared.as_ref().map(|old| {
            LanguageTextPriorRevision::new(
                old.tape().source().material().revision().clone(),
                *old.tape().source().sequence(),
            )
            .unwrap()
        });
        let source = LanguageTextRevision::new(
            finality,
            LanguageText::new(
                LanguageTextId::new("stream/v2/travis-hello".into()).unwrap(),
                profile.language().clone(),
                LanguageTextRevisionId::new(format!("stream/{sequence}")).unwrap(),
                text.into(),
            )
            .unwrap(),
            prior,
            profile.provenance().clone(),
            sequence,
            stable,
        )
        .unwrap();
        let next = prepare_lexical_tape(&source, profile, prepared.as_ref()).unwrap();
        history.push(
            source
                .clone()
                .into_structured()
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
        if sequence == 0 {
            let available = LanguageParserAvailableLexical::new(next.tape().clone(), 0).unwrap();
            let waiting = session.available(&available);
            assert_eq!(waiting["waiting"], true);
            eprintln!("STREAM_INITIAL_WAIT {waiting}");
        } else {
            let forms = if sequence == 1 {
                vec!["Travis".into()]
            } else {
                template.forms.clone()
            };
            let row = joint::Sentence {
                id: format!("retained-stream/{sequence}"),
                forms,
                pos: if sequence == 1 {
                    vec![11]
                } else {
                    template.pos.clone()
                },
                heads: if sequence == 1 {
                    vec![4]
                } else {
                    template.heads.clone()
                },
                relations: if sequence == 1 {
                    vec!["root".into()]
                } else {
                    template.relations.clone()
                },
                text: Some(text.into()),
            };
            let count = row.forms.len() as u64;
            inputs.push(Ok(LanguageParserJointLexical::new(
                next.tape().clone(),
                count,
            )
            .unwrap()));
            rows.push(row);
        }
        prepared = Some(next);
    }
    runtime::evaluate_observed(Some("retained-stream"), rows, inputs, &mut session);
    assert_eq!(session.flows.flows[0].sequence, 4);
    assert_eq!(session.flows.flows[1].sequence, 2);
    assert!(session.waits > 0);
    if let Ok(path) = std::env::var("CONDUIT_PARSER_STREAM_HISTORY_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&history).unwrap()).unwrap();
    }
}

#[test]
#[ignore = "actual retained partial-source learned stream evidence; cold finite Plan preparation measured"]
fn source_commit_then_append_preserves_native_dependency_and_head_choice() {
    let template = joint::Sentence {
        id: "protected-stream-profile".into(),
        forms: vec!["Travis".into(), "Hello".into(), "friend".into()],
        pos: vec![11, 6, 7],
        heads: vec![1, 4, 1],
        relations: vec!["vocative".into(), "root".into(), "obj".into()],
        text: None,
    };
    let reference = joint::lexical(&template).unwrap();
    let profile = reference.tape().profile();
    let mut prepared: Option<PreparedLexicalTape> = None;
    let mut history = Vec::new();
    let mut rows = Vec::new();
    let mut inputs = Vec::new();
    let mut session = Session::stable(profile);
    for (sequence, text, finality, stable) in [
        (0, "Tr", LanguageTextFinality::Partial, None),
        (1, "Travis ", LanguageTextFinality::Partial, Some(7)),
        (2, "Travis Hello ", LanguageTextFinality::Partial, Some(13)),
        (
            3,
            "Travis Hello friend ",
            LanguageTextFinality::Partial,
            Some(20),
        ),
        (
            4,
            "Travis Hello friend ",
            LanguageTextFinality::Final,
            Some(20),
        ),
    ] {
        let prior = prepared.as_ref().map(|old| {
            LanguageTextPriorRevision::new(
                old.tape().source().material().revision().clone(),
                *old.tape().source().sequence(),
            )
            .unwrap()
        });
        let source = LanguageTextRevision::new(
            finality,
            LanguageText::new(
                LanguageTextId::new("stream/v2/protected-travis-hello-friend".into()).unwrap(),
                profile.language().clone(),
                LanguageTextRevisionId::new(format!("protected-stream/{sequence}")).unwrap(),
                text.into(),
            )
            .unwrap(),
            prior,
            profile.provenance().clone(),
            sequence,
            stable,
        )
        .unwrap();
        let next = prepare_lexical_tape(&source, profile, prepared.as_ref()).unwrap();
        history.push(
            source
                .clone()
                .into_structured()
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
        if sequence == 0 {
            let available = LanguageParserAvailableLexical::new(next.tape().clone(), 0).unwrap();
            let waiting = session.available(&available);
            assert_eq!(waiting["waiting"], true);
            eprintln!("STREAM_INITIAL_WAIT {waiting}");
        } else {
            let count = if sequence == 1 {
                1
            } else if sequence == 2 {
                2
            } else {
                3
            };
            let row = joint::Sentence {
                id: format!("protected-stream/{sequence}"),
                forms: template.forms[..count].to_vec(),
                pos: template.pos[..count].to_vec(),
                heads: if count == 1 {
                    vec![4]
                } else {
                    template.heads[..count].to_vec()
                },
                relations: if count == 1 {
                    vec!["root".into()]
                } else {
                    template.relations[..count].to_vec()
                },
                text: Some(text.into()),
            };
            let count = row.forms.len() as u64;
            inputs.push(Ok(LanguageParserJointLexical::new(
                next.tape().clone(),
                count,
            )
            .unwrap()));
            rows.push(row);
        }
        prepared = Some(next);
    }
    runtime::evaluate_observed(Some("protected-stream"), rows, inputs, &mut session);
    assert_eq!(session.flows.flows[0].sequence, 5);
    assert_eq!(session.flows.flows[7].sequence, 3);
    assert_eq!(session.committed(), 3);
    assert_eq!(session.committed_snapshots, [0, 1, 1, 3]);
    assert!(session.waits > 0);
    if let Ok(path) = std::env::var("CONDUIT_PARSER_STREAM_HISTORY_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&history).unwrap()).unwrap();
    }
}

#[test]
#[ignore = "explicit cold Source preparation proof"]
fn retained_rebase_source_prepares() {
    let _ = parser_kernel::Blueprint::prepare(retained::source(), "language-parser-joint-rebase");
}

#[test]
fn inactive_placeholder_retains_native_selected_unread_bound() {
    let mut fixture = fixture::Fixture::new();
    let ty = LanguageParserState::semantic_type().unwrap();
    let raw = fixture::replace(
        &joint::retype(&ty, &fixture.initial(2)),
        "unread",
        fixture::number(fixture::field_type(&ty, "unread"), 1),
    );
    let state = LanguageParserState::from_structured(raw).unwrap();
    let inactive = joint::inactive(&state);
    assert_eq!(*inactive.selected(), 1);
    assert_eq!(inactive.hypothesis().parser().state(), &state);
    assert!(!inactive.hypothesis().parser().active());
}
