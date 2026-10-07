#![cfg(feature = "semantic-bindings")]
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::committed_token_role::*;

#[test]
#[ignore = "requires the actual committed speech receipt and exact revision history"]
fn committed_role_retains_custody_and_refuses_a_different_lexical_revision() {
    let receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var("CONDUIT_COMMITTED_SPEECH_RECEIPT").unwrap()).unwrap(),
    )
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(
        receipt["graph_receipt"]["committed_dependency_admission_bytes"].clone(),
    )
    .unwrap();
    let committed = LanguageParserCommittedDependencyAdmission::decode(&bytes).unwrap();
    let uncommitted_runtime = LanguageParserJointRuntimeBeam::new(
        committed.commit().fact().query().beam().clone(),
        *committed.runtime().selected(),
    )
    .unwrap();
    assert!(LanguageParserCommittedDependencyAdmission::new(
        committed.admission().clone(),
        committed.commit().clone(),
        uncommitted_runtime,
    )
    .is_err());
    let query = committed.admission().fact().query();
    let revisions: Vec<Vec<u8>> = serde_json::from_slice(
        &std::fs::read(std::env::var("CONDUIT_STABLE_PARSER_HISTORY").unwrap()).unwrap(),
    )
    .unwrap();
    let mut previous = None;
    let different_revision = LanguageTextRevision::decode(&revisions[0]).unwrap();
    let different = prepare_lexical_tape(
        &different_revision,
        query.beam().lexical().tape().profile(),
        None,
    )
    .unwrap();
    assert_ne!(different.tape(), query.beam().lexical().tape());
    for bytes in revisions {
        let revision = LanguageTextRevision::decode(&bytes).unwrap();
        let lexical = prepare_lexical_tape(
            &revision,
            query.beam().lexical().tape().profile(),
            previous.as_ref(),
        )
        .unwrap();
        if lexical.tape() == query.beam().lexical().tape() {
            let role = prepare_committed_token_role(&lexical, &committed).unwrap();
            assert_eq!(role.committed(), &committed);
            let request: Vec<u8> =
                serde_json::from_value(receipt["token_participation"][0]["request_bytes"].clone())
                    .unwrap();
            let result: Vec<u8> =
                serde_json::from_value(receipt["token_participation"][0]["result_bytes"].clone())
                    .unwrap();
            assert_eq!(role.role().request().clone().encode().unwrap(), request);
            assert_eq!(role.role().result().clone().encode().unwrap(), result);
            assert!(matches!(
                prepare_committed_token_role(&different, &committed),
                Err(CommittedTokenRoleRefusal::LexicalTape)
            ));
            return;
        }
        previous = Some(lexical);
    }
    panic!("committed lexical revision missing from exact history");
}
