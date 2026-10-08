#![cfg(feature = "semantic-bindings")]
//! Continue retained actual Source commitments; never rerun the model or infer
//! authority from a display label, protected edge, or spoken coverage receipt.
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::committed_token_role::*;

fn bounded_json(path: &str) -> serde_json::Value {
    assert!(std::fs::metadata(path).unwrap().len() <= 32 * 1024 * 1024);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
#[ignore = "requires immutable actual four-revision parser events and acquisition"]
fn actual_final_word_stream_commits_both_spoken_token_roles() {
    let path = std::env::var("CONDUIT_WORD_STREAM_EVENTS").unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() <= 32 * 1024 * 1024);
    let events: Vec<serde_json::Value> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let acquisition = bounded_json(&std::env::var("CONDUIT_WORD_STREAM_ACQUISITION").unwrap());
    let snapshot = events
        .iter()
        .find(|row| {
            row["event"] == "snapshot"
                && row["text"] == "Hello, Travis."
                && row["source_sequence"] == 3
                && row["committed"] == 3
        })
        .expect("actual final Source snapshot must cover both spoken words");
    let decode_bytes =
        |value: &serde_json::Value| -> Vec<u8> { serde_json::from_value(value.clone()).unwrap() };
    let final_runtime =
        LanguageParserJointRuntimeBeam::decode(&decode_bytes(&snapshot["beam_bytes"])).unwrap();
    assert_eq!(
        *final_runtime
            .beam()
            .candidate0()
            .parser()
            .state()
            .committed(),
        3
    );
    let mut previous = None;
    for bytes in acquisition["source_revision_history_bytes"]
        .as_array()
        .unwrap()
    {
        let revision = LanguageTextRevision::decode(&decode_bytes(bytes)).unwrap();
        previous = Some(
            prepare_lexical_tape(
                &revision,
                final_runtime.beam().lexical().tape().profile(),
                previous.as_ref(),
            )
            .unwrap(),
        );
    }
    let lexical = previous.unwrap();
    assert_eq!(lexical.tape(), final_runtime.beam().lexical().tape());
    let earlier_revision = LanguageTextRevision::decode(&decode_bytes(
        &acquisition["source_revision_history_bytes"][0],
    ))
    .unwrap();
    let earlier = prepare_lexical_tape(&earlier_revision, lexical.tape().profile(), None).unwrap();
    let mut admitted = Vec::new();
    for dependent in [0_u64, 2] {
        let fact = snapshot["stable_fact_bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|bytes| LanguageParserJointStableFact::decode(&decode_bytes(bytes)).unwrap())
            .find(|fact| *fact.query().dependent() == dependent)
            .expect("retain original actual stable query for each committed word");
        let query = fact.query();
        assert_eq!(query.beam().basis(), final_runtime.beam().basis());
        let state = query.beam().candidate0().parser().state();
        assert_eq!(*state.committed(), dependent);
        let relation = if dependent == 0 {
            state.relation0()
        } else {
            state.relation2()
        };
        let head = state.heads()[dependent as usize];
        let token = |ordinal: usize| {
            LanguageAnalysisTokenRef::new(
                query.beam().basis().analysis_revision().clone(),
                query.beam().lexical().tape().tokens()[ordinal]
                    .identity()
                    .clone(),
            )
            .unwrap()
        };
        let governor = if head == 4 {
            LanguageDependencyHead::root()
        } else {
            let reference = token(head as usize);
            LanguageDependencyHead::token(reference.revision().clone(), reference.token().clone())
                .unwrap()
        };
        let subtype = (!relation.subtype().get().is_empty())
            .then(|| LanguageDependencySubtype::new(relation.subtype().get().clone()).unwrap());
        let arc = LanguageDependencyArc::new(
            token(dependent as usize),
            governor,
            LanguageDependencyRelation::new(*relation.base(), subtype).unwrap(),
        )
        .unwrap();
        let admission = LanguageParserStableDependencyAdmission::new(
            arc,
            fact.clone(),
            head,
            relation.subtype().clone(),
        )
        .unwrap();
        let commit = LanguageParserJointCommitQuery::new(fact).unwrap();
        let row = events
            .iter()
            .find(|row| {
                row["event"] == "dependency-commit"
                    && row["source_revision"] == snapshot["source_revision"]
                    && row["analysis_revision"] == snapshot["analysis_revision"]
                    && row["committed"] == dependent + 1
            })
            .expect("exact Source commit output");
        let runtime =
            LanguageParserJointRuntimeBeam::decode(&decode_bytes(&row["native_runtime_bytes"]))
                .unwrap();
        let uncommitted = LanguageParserJointRuntimeBeam::new(
            commit.fact().query().beam().clone(),
            *runtime.selected(),
        )
        .unwrap();
        assert!(LanguageParserCommittedDependencyAdmission::new(
            admission.clone(),
            commit.clone(),
            uncommitted,
        )
        .is_err());
        let committed =
            LanguageParserCommittedDependencyAdmission::new(admission, commit, runtime).unwrap();
        let role = prepare_committed_token_role(&lexical, &committed).unwrap();
        assert_eq!(role.committed(), &committed);
        assert!(matches!(
            role.role().result().role(),
            conduit_speech::semantic::SpeechTextTokenRole::Spoken
        ));
        assert!(matches!(
            prepare_committed_token_role(&earlier, &committed),
            Err(CommittedTokenRoleRefusal::LexicalTape)
        ));
        admitted.push(serde_json::json!({
            "dependent": dependent,
            "committed_dependency_admission_bytes": committed.clone().encode().unwrap(),
            "role_request_bytes": role.role().request().clone().encode().unwrap(),
            "role_result_bytes": role.role().result().clone().encode().unwrap(),
        }));
    }
    if let Ok(path) = std::env::var("CONDUIT_WORD_STREAM_COMMITTED_ROLES_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"speech/actual-word-stream-committed-roles@1",
            "source_text":"Hello, Travis.", "spoken_ordinals":[0,2],
            "commitments":admitted, "lexical_tape_bytes":lexical.tape().clone().encode().unwrap(),
            "source_revision_history_bytes":acquisition["source_revision_history_bytes"],
            "generated_audio":false, "played_commitment":false,
            "shared_ir_5212_completion":false,
        })).unwrap()).unwrap();
    }
}
