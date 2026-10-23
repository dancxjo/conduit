//! Graph replies freeze the selected model until every preparation phase ends.
use super::*;
fn complete_phase(state: &mut State, transfer: &mut [u8], phase: crate::wire::GraphPhase) {
    let limit = match phase {
        crate::wire::GraphPhase::Serialize => data::GRAPH_SERIALIZE_TURNS,
        crate::wire::GraphPhase::Validate => data::GRAPH_VALIDATE_TURNS,
    };
    let mut progress = crate::wire::GraphProgress::new(phase, limit).unwrap();
    loop {
        let response = state
            .execute(progress.request().unwrap(), transfer)
            .unwrap();
        if progress.accept(response).unwrap() {
            break;
        }
    }
}

fn pending() -> (State, Vec<u8>) {
    let mut state = State::default();
    call(
        &mut state,
        &manifest([Some(InitSlot::Tour), None, None, None]),
    )
    .unwrap();
    call(&mut state, &Command::PrepareTour { slot: 0 }).unwrap();
    call(&mut state, &Command::FinishInit).unwrap();
    begin_graph(state)
}

fn begin_graph(mut state: State) -> (State, Vec<u8>) {
    let input = data::encode_command(&Command::Graph { slot: 0 }).unwrap();
    let mut transfer = vec![0; crate::memory::TRANSFER_BYTES];
    state
        .execute(
            Request::Begin {
                bytes: input.len() as u32,
            },
            &mut transfer,
        )
        .unwrap();
    transfer[..input.len()].copy_from_slice(&input);
    state
        .execute(
            Request::Write {
                offset: 0,
                bytes: input.len() as u32,
            },
            &mut transfer,
        )
        .unwrap();
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Ok(Response::Ready)
    );
    (state, transfer)
}

#[test]
fn ordered_graph_phases_publish_only_the_final_complete_blob() {
    let (mut state, mut transfer) = pending();
    assert!(state.outgoing.is_empty());
    complete_phase(
        &mut state,
        &mut transfer,
        crate::wire::GraphPhase::Serialize,
    );
    assert!(state.outgoing.is_empty());
    complete_phase(&mut state, &mut transfer, crate::wire::GraphPhase::Validate);
    assert!(state.outgoing.is_empty());
    let Ok(Response::Blob { bytes }) =
        state.execute(Request::GraphPack { offset: 0 }, &mut transfer)
    else {
        panic!("small reply must finish in one packing turn");
    };
    assert_eq!(bytes as usize, state.outgoing.len());
    assert!(state.graph_reply.is_none());
    assert!(matches!(
        data::decode_reply(&state.outgoing),
        Ok(Reply::Graph { graph: None })
    ));
    assert_eq!(
        state.execute(Request::Begin { bytes: 1 }, &mut transfer),
        Err(Refusal::Order)
    );
    terminal(&mut state);
}

#[test]
fn premature_reads_interleaving_duplicate_and_skipped_phases_are_terminal() {
    for (phase, request) in [
        (0, Request::Begin { bytes: 1 }),
        (
            0,
            Request::Read {
                offset: 0,
                maximum: 1,
            },
        ),
        (0, Request::GraphValidate { cursor: 0 }),
        (0, Request::GraphPack { offset: 0 }),
        (1, Request::GraphSerialize { cursor: 0 }),
        (1, Request::GraphPack { offset: 0 }),
        (1, Request::Commit),
        (2, Request::GraphValidate { cursor: 0 }),
        (2, Request::GraphPack { offset: 1 }),
        (
            2,
            Request::Read {
                offset: 0,
                maximum: 1,
            },
        ),
    ] {
        let (mut state, mut transfer) = pending();
        if phase >= 1 {
            complete_phase(
                &mut state,
                &mut transfer,
                crate::wire::GraphPhase::Serialize,
            );
        }
        if phase >= 2 {
            complete_phase(&mut state, &mut transfer, crate::wire::GraphPhase::Validate);
        }
        assert_eq!(state.execute(request, &mut transfer), Err(Refusal::Order));
        terminal(&mut state);
    }
    for request in [
        Request::GraphSerialize { cursor: 0 },
        Request::GraphValidate { cursor: 0 },
        Request::GraphPack { offset: 0 },
    ] {
        let mut state = State::default();
        let mut transfer = vec![0; crate::memory::TRANSFER_BYTES];
        assert_eq!(state.execute(request, &mut transfer), Err(Refusal::Order));
        terminal(&mut state);
    }
}

#[test]
fn new_phase_wire_tags_roundtrip_and_reject_reserved_arguments() {
    let envelope = crate::wire::Envelope {
        instance: 1,
        sequence: 2,
    };
    let mut bytes = [0; crate::wire::BYTES];
    for request in [
        Request::GraphSerialize { cursor: 0 },
        Request::GraphValidate { cursor: 0 },
        Request::GraphPack { offset: 4096 },
    ] {
        crate::wire::encode_request(envelope, request, &mut bytes).unwrap();
        assert_eq!(crate::wire::decode_request(&bytes), Ok((envelope, request)));
        bytes[36] = 1;
        assert_eq!(crate::wire::decode_request(&bytes), Err(Refusal::Malformed));
    }
}

fn populated() -> (State, Vec<u8>) {
    #[derive(serde::Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<Initial>; crate::SLOTS],
        },
    }
    let Fixture::Init { applications } =
        data::decode(include_bytes!("../../../tests/fixtures/startup.catz")).unwrap();
    let models = Models::new(applications).unwrap();
    assert!(models.graph(0).is_some());
    begin_graph(State {
        models: Some(models),
        ..State::default()
    })
}
#[test]
fn actual_populated_graph_requires_correlated_progress_before_exact_blob() {
    let (mut state, mut transfer) = populated();
    let expected = data::encode(&Reply::Graph {
        graph: state.models.as_ref().unwrap().graph(0).cloned(),
    })
    .unwrap();
    for phase in [
        crate::wire::GraphPhase::Serialize,
        crate::wire::GraphPhase::Validate,
    ] {
        let limit = match phase {
            crate::wire::GraphPhase::Serialize => data::GRAPH_SERIALIZE_TURNS,
            crate::wire::GraphPhase::Validate => data::GRAPH_VALIDATE_TURNS,
        };
        let mut progress = crate::wire::GraphProgress::new(phase, limit).unwrap();
        let mut turns = 0;
        loop {
            let response = state
                .execute(progress.request().unwrap(), &mut transfer)
                .unwrap();
            turns += 1;
            assert!(state.outgoing.is_empty());
            if progress.accept(response).unwrap() {
                break;
            }
        }
        assert!(
            turns > 1,
            "populated production graph must exercise continuation"
        );
    }
    let mut complete = false;
    for offset in (0..data::MAX_DECODED_JSON_BYTES).step_by(data::GRAPH_PACK_BYTES) {
        if matches!(
            state.execute(
                Request::GraphPack {
                    offset: offset as u32
                },
                &mut transfer
            ),
            Ok(Response::Blob { .. })
        ) {
            complete = true;
            break;
        }
    }
    assert!(complete && state.graph_reply.is_none());
    assert_eq!(
        data::encode(&data::decode_reply(&state.outgoing).unwrap()).unwrap(),
        expected
    );
}
#[test]
fn populated_progress_rejects_stale_skipped_phase_and_interleaved_requests() {
    for request in [
        Request::GraphSerialize { cursor: 0 },
        Request::GraphSerialize { cursor: 2 },
        Request::GraphValidate { cursor: 0 },
        Request::GraphPack { offset: 0 },
        Request::Read {
            offset: 0,
            maximum: 1,
        },
        Request::Commit,
        Request::Begin { bytes: 1 },
    ] {
        let (mut state, mut transfer) = populated();
        assert_eq!(
            state.execute(Request::GraphSerialize { cursor: 0 }, &mut transfer),
            Ok(Response::Progress {
                phase: crate::wire::GraphPhase::Serialize,
                cursor: 1
            })
        );
        assert_eq!(state.execute(request, &mut transfer), Err(Refusal::Order));
        assert!(state.outgoing.is_empty());
        terminal(&mut state);
    }
    for cursor in [0, 2] {
        let (mut state, mut transfer) = populated();
        complete_phase(
            &mut state,
            &mut transfer,
            crate::wire::GraphPhase::Serialize,
        );
        assert_eq!(
            state.execute(Request::GraphValidate { cursor: 0 }, &mut transfer),
            Ok(Response::Progress {
                phase: crate::wire::GraphPhase::Validate,
                cursor: 1
            })
        );
        assert_eq!(
            state.execute(Request::GraphValidate { cursor }, &mut transfer),
            Err(Refusal::Order)
        );
        terminal(&mut state);
    }
}
#[test]
fn phase_turn_ceiling_and_invalid_frozen_validation_are_terminal_without_blob() {
    let (mut state, mut transfer) = populated();
    let Some(GraphReply::Serialize { turn, .. }) = state.graph_reply.as_mut() else {
        panic!("serialization")
    };
    *turn = data::GRAPH_SERIALIZE_TURNS - 1;
    assert_eq!(
        state.execute(
            Request::GraphSerialize {
                cursor: data::GRAPH_SERIALIZE_TURNS - 1
            },
            &mut transfer
        ),
        Err(Refusal::Bounds)
    );
    terminal(&mut state);
    let (mut state, mut transfer) = populated();
    complete_phase(
        &mut state,
        &mut transfer,
        crate::wire::GraphPhase::Serialize,
    );
    let Some(GraphReply::Validate { turn, .. }) = state.graph_reply.as_mut() else {
        panic!("validation")
    };
    *turn = data::GRAPH_VALIDATE_TURNS - 1;
    assert_eq!(
        state.execute(
            Request::GraphValidate {
                cursor: data::GRAPH_VALIDATE_TURNS - 1
            },
            &mut transfer
        ),
        Err(Refusal::Bounds)
    );
    terminal(&mut state);
    let (mut state, mut transfer) = populated();
    complete_phase(
        &mut state,
        &mut transfer,
        crate::wire::GraphPhase::Serialize,
    );
    state.graph_reply = Some(GraphReply::Validate {
        length: 1,
        turn: 0,
        validation: data::GraphValidation::new(1).unwrap(),
    });
    assert_eq!(
        state.execute(Request::GraphValidate { cursor: 0 }, &mut transfer),
        Err(Refusal::Malformed)
    );
    assert!(state.outgoing.is_empty());
    terminal(&mut state);
}
