//! The readonly query has one private pending owner and exactly two commands.
use super::*;
use crate::graph_facts::{self, Scope};
fn projection() -> Command {
    let graph = small_target().graph;
    #[derive(serde::Deserialize)]
    enum Fixture {
        Init { applications: [Option<Initial>; 4] },
    }
    let Fixture::Init { mut applications } =
        data::decode(include_bytes!("../../../tests/fixtures/startup.catz")).unwrap();
    let Some(Initial::Tutorial { evidence }) = applications[1].take() else {
        panic!("fixture")
    };
    let scope = Scope::new(
        "host/projection".into(),
        "boot/projection".into(),
        evidence.body_id,
        7,
        &graph,
        false,
    )
    .unwrap();
    Command::PrepareGraphProjection { graph, scope }
}
#[test]
fn cold_prepare_finish_preserves_exact_scope_graph_facts_and_duplicate_finish_refuses() {
    let command = projection();
    let Command::PrepareGraphProjection { graph, scope } = &command else {
        unreachable!()
    };
    let expected = graph_facts::project(graph, scope.clone()).unwrap();
    let mut state = State::default();
    assert!(matches!(call(&mut state, &command), Ok(Reply::Prepared)));
    assert!(state.models.is_none());
    assert!(state.projection.is_some());
    let Reply::GraphFacts { contribution } =
        call(&mut state, &Command::FinishGraphProjection).unwrap()
    else {
        panic!("facts")
    };
    assert_eq!(
        data::encode(&contribution).unwrap(),
        data::encode(&expected).unwrap()
    );
    assert_eq!(contribution.scope, *scope);
    assert!(state.projection.is_none());
    assert!(matches!(
        call(&mut state, &Command::FinishGraphProjection),
        Err(Refusal::Order)
    ));
    terminal(&mut state);
}
#[test]
fn finish_before_prepare_and_all_interleaved_commands_terminally_drop_the_query() {
    let mut state = State::default();
    assert!(matches!(
        call(&mut state, &Command::FinishGraphProjection),
        Err(Refusal::Order)
    ));
    terminal(&mut state);
    for other in [
        projection(),
        manifest([None; 4]),
        Command::FinishInit,
        Command::Graph { slot: 0 },
        Command::Apply {
            slot: 0,
            input: vec![],
        },
        Command::Select { slot: 0, target: 1 },
    ] {
        let mut state = State::default();
        assert!(matches!(
            call(&mut state, &projection()),
            Ok(Reply::Prepared)
        ));
        assert!(matches!(call(&mut state, &other), Err(Refusal::Order)));
        terminal(&mut state);
    }
}
#[test]
fn forged_digest_never_publishes_facts_and_invalid_basis_never_prepares() {
    let mut command = projection();
    let Command::PrepareGraphProjection { scope, .. } = &mut command else {
        unreachable!()
    };
    scope.graph_digest[0] ^= 1;
    let mut state = State::default();
    assert!(matches!(call(&mut state, &command), Ok(Reply::Prepared)));
    assert!(matches!(
        call(&mut state, &Command::FinishGraphProjection),
        Err(Refusal::Bounds)
    ));
    terminal(&mut state);
    let mut command = projection();
    let Command::PrepareGraphProjection { scope, .. } = &mut command else {
        unreachable!()
    };
    scope.owner_revision = 0;
    let mut state = State::default();
    assert!(matches!(call(&mut state, &command), Err(Refusal::Bounds)));
    terminal(&mut state);
}
#[test]
fn unread_prepared_reply_cannot_be_overwritten_by_finish() {
    let mut state = State::default();
    let input = data::encode_command(&projection()).unwrap();
    let mut transfer = vec![0; crate::memory::TRANSFER_BYTES];
    state
        .execute(
            Request::Begin {
                bytes: input.len() as u32,
            },
            &mut transfer,
        )
        .unwrap();
    for (index, chunk) in input.chunks(transfer.len()).enumerate() {
        transfer[..chunk.len()].copy_from_slice(chunk);
        state
            .execute(
                Request::Write {
                    offset: (index * transfer.len()) as u32,
                    bytes: chunk.len() as u32,
                },
                &mut transfer,
            )
            .unwrap();
    }
    assert!(matches!(
        state.execute(Request::Commit, &mut transfer),
        Ok(Response::Blob { .. })
    ));
    assert!(matches!(
        data::decode_reply(&state.outgoing),
        Ok(Reply::Prepared)
    ));
    assert!(matches!(
        state.execute(Request::Begin { bytes: 1 }, &mut transfer),
        Err(Refusal::Order)
    ));
    terminal(&mut state);
}
