//! Graph replies freeze the selected model until every preparation phase ends.
use super::*;

fn pending() -> (State, Vec<u8>) {
    let mut state = State::default();
    call(
        &mut state,
        &manifest([Some(InitSlot::Tour), None, None, None]),
    )
    .unwrap();
    call(&mut state, &Command::PrepareTour { slot: 0 }).unwrap();
    call(&mut state, &Command::FinishInit).unwrap();
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
    assert_eq!(
        state.execute(Request::GraphSerialize, &mut transfer),
        Ok(Response::Ready)
    );
    assert!(state.outgoing.is_empty());
    assert_eq!(
        state.execute(Request::GraphValidate, &mut transfer),
        Ok(Response::Ready)
    );
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
        (0, Request::GraphValidate),
        (0, Request::GraphPack { offset: 0 }),
        (1, Request::GraphSerialize),
        (1, Request::GraphPack { offset: 0 }),
        (1, Request::Commit),
        (2, Request::GraphValidate),
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
            state
                .execute(Request::GraphSerialize, &mut transfer)
                .unwrap();
        }
        if phase >= 2 {
            state
                .execute(Request::GraphValidate, &mut transfer)
                .unwrap();
        }
        assert_eq!(state.execute(request, &mut transfer), Err(Refusal::Order));
        terminal(&mut state);
    }
    for request in [
        Request::GraphSerialize,
        Request::GraphValidate,
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
        Request::GraphSerialize,
        Request::GraphValidate,
        Request::GraphPack { offset: 4096 },
    ] {
        crate::wire::encode_request(envelope, request, &mut bytes).unwrap();
        assert_eq!(crate::wire::decode_request(&bytes), Ok((envelope, request)));
        bytes[36] = 1;
        assert_eq!(crate::wire::decode_request(&bytes), Err(Refusal::Malformed));
    }
}
