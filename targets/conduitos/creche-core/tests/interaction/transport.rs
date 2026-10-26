//! Observable transaction contract; preparation is not permission to apply.
use super::fixture;
use conduitos_creche_core::{
    data::{self, Initialize, InteractionRequest},
    state::State,
    wire::{Event, InputKind, MAX_BLOB_BYTES, Refusal, Request, Response, TRANSFER_BYTES},
};

fn write(
    state: &mut State,
    transfer: &mut [u8],
    offset: usize,
    bytes: &[u8],
) -> Result<Response, Refusal> {
    transfer.fill(0);
    transfer[..bytes.len()].copy_from_slice(bytes);
    state.execute(
        Request::Write {
            offset: offset as u32,
            window_offset: 0,
            bytes: bytes.len() as u32,
        },
        transfer,
    )
}
fn begin_and_write(state: &mut State, transfer: &mut [u8], kind: InputKind, bytes: &[u8]) {
    assert!(matches!(
        state.execute(
            Request::Begin {
                kind,
                total: bytes.len() as u32
            },
            transfer
        ),
        Ok(Response::Ready { .. })
    ));
    for (index, chunk) in bytes.chunks(TRANSFER_BYTES as usize).enumerate() {
        assert!(matches!(
            write(state, transfer, index * TRANSFER_BYTES as usize, chunk),
            Ok(Response::Ready { .. })
        ));
    }
}
fn initialized() -> (State, Vec<u8>, InteractionRequest) {
    let (draft, input) = fixture();
    let initialization = data::encode(&Initialize {
        uuid: "550e8400-e29b-41d4-a716-446655440000".into(),
        choices: draft.choices().to_vec(),
        allow_idle_body: false,
    })
    .unwrap();
    let mut state = State::default();
    let mut transfer = vec![0; TRANSFER_BYTES as usize];
    begin_and_write(
        &mut state,
        &mut transfer,
        InputKind::Initialize,
        &initialization,
    );
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Ok(Response::Ready {
            revision: input.revision
        })
    );
    (state, transfer, input)
}
fn commit_valid(state: &mut State, transfer: &mut [u8], input: &InteractionRequest) {
    begin_and_write(
        state,
        transfer,
        InputKind::FaceInteraction,
        &data::encode_interaction(input).unwrap(),
    );
    assert_eq!(
        state.execute(Request::Commit, transfer),
        Ok(Response::Ready {
            revision: input.revision + 1
        })
    );
}
#[test]
fn final_write_is_only_preparation_and_transfer_changes_cannot_change_owned_input() {
    let (mut state, mut transfer, input) = initialized();
    let bytes = data::encode_interaction(&input).unwrap();
    begin_and_write(
        &mut state,
        &mut transfer,
        InputKind::FaceInteraction,
        &bytes,
    );
    assert_eq!(state.revision(), Ok(input.revision));
    transfer.fill(0xff);
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Ok(Response::Ready {
            revision: input.revision + 1
        })
    );
    assert_eq!(state.revision(), Ok(input.revision + 1));
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Err(Refusal::Order)
    );
    // A new transaction must perform current admission, not replay prior success.
    begin_and_write(
        &mut state,
        &mut transfer,
        InputKind::FaceInteraction,
        &bytes,
    );
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Err(Refusal::StaleFace)
    );
    assert_eq!(state.revision(), Ok(input.revision + 1));
}
#[test]
fn completed_malformed_input_defers_error_to_commit_and_blocks_other_operations() {
    let (mut state, mut transfer, input) = initialized();
    let mut malformed = data::encode_interaction(&input).unwrap();
    malformed.pop();
    begin_and_write(
        &mut state,
        &mut transfer,
        InputKind::FaceInteraction,
        &malformed,
    );
    let guards = [
        Request::Begin {
            kind: InputKind::FaceInteraction,
            total: 1,
        },
        Request::Read {
            offset: 0,
            maximum: 1,
        },
        Request::Snapshot {
            revision: input.revision,
        },
        Request::Selection {
            revision: input.revision,
        },
        Request::Apply {
            revision: input.revision,
            event: Event::Change,
            action: "",
            value: "",
        },
        Request::NativeKey {
            revision: input.revision,
            event: [0; 3],
        },
        Request::AllowIdle,
        Request::HomeInit,
        Request::HomeReset { revision: 1 },
        Request::HomeRefusal {
            revision: 1,
            code: 1,
        },
    ];
    for request in guards {
        assert_eq!(state.execute(request, &mut transfer), Err(Refusal::Order));
    }
    assert_eq!(state.revision(), Ok(input.revision));
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Err(Refusal::Malformed)
    );
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Err(Refusal::Order)
    );
    // Decode failure must not be cached into the following complete request.
    commit_valid(&mut state, &mut transfer, &input);
}
#[test]
fn incomplete_commit_consumes_transaction_and_cannot_reuse_partial_preparation() {
    let (mut state, mut transfer, input) = initialized();
    let bytes = data::encode_interaction(&input).unwrap();
    assert!(bytes.len() > 1);
    state
        .execute(
            Request::Begin {
                kind: InputKind::FaceInteraction,
                total: bytes.len() as u32,
            },
            &mut transfer,
        )
        .unwrap();
    write(&mut state, &mut transfer, 0, &bytes[..1]).unwrap();
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Err(Refusal::Order)
    );
    assert_eq!(
        write(&mut state, &mut transfer, 1, &bytes[1..2]),
        Err(Refusal::Order)
    );
    assert_eq!(state.revision(), Ok(input.revision));
    commit_valid(&mut state, &mut transfer, &input);
}
#[test]
fn malformed_writes_and_repeated_final_write_cannot_apply_or_replace_prepared_input() {
    let (mut state, mut transfer, input) = initialized();
    let bytes = data::encode_interaction(&input).unwrap();
    state
        .execute(
            Request::Begin {
                kind: InputKind::FaceInteraction,
                total: bytes.len() as u32,
            },
            &mut transfer,
        )
        .unwrap();
    for (request, refusal) in [
        (
            Request::Write {
                offset: 0,
                window_offset: 0,
                bytes: 0,
            },
            Refusal::Order,
        ),
        (
            Request::Write {
                offset: 1,
                window_offset: 0,
                bytes: 1,
            },
            Refusal::Order,
        ),
        (
            Request::Write {
                offset: u32::MAX,
                window_offset: 0,
                bytes: 1,
            },
            Refusal::Bounds,
        ),
        (
            Request::Write {
                offset: 0,
                window_offset: TRANSFER_BYTES,
                bytes: 1,
            },
            Refusal::Bounds,
        ),
    ] {
        assert_eq!(state.execute(request, &mut transfer), Err(refusal));
    }
    for (index, chunk) in bytes.chunks(TRANSFER_BYTES as usize).enumerate() {
        write(
            &mut state,
            &mut transfer,
            index * TRANSFER_BYTES as usize,
            chunk,
        )
        .unwrap();
    }
    let last = ((bytes.len() - 1) / TRANSFER_BYTES as usize) * TRANSFER_BYTES as usize;
    assert_eq!(
        write(&mut state, &mut transfer, last, &bytes[last..]),
        Err(Refusal::Order)
    );
    assert_eq!(
        state.execute(
            Request::Write {
                offset: bytes.len() as u32,
                window_offset: 0,
                bytes: 1
            },
            &mut transfer
        ),
        Err(Refusal::Bounds)
    );
    assert_eq!(
        state.execute(
            Request::Write {
                offset: bytes.len() as u32,
                window_offset: 0,
                bytes: 0
            },
            &mut transfer
        ),
        Err(Refusal::Order)
    );
    assert_eq!(state.revision(), Ok(input.revision));
    assert_eq!(
        state.execute(Request::Commit, &mut transfer),
        Ok(Response::Ready {
            revision: input.revision + 1
        })
    );
}
#[test]
fn canonical_plan_placement_and_stale_revision_checks_still_happen_at_commit() {
    for variant in 0..3 {
        let (mut state, mut transfer, mut input) = initialized();
        match variant {
            0 => input.show.planned_mask.plan.plan_id = "plan/foreign".into(),
            1 => {
                input.show.planned_mask.plan.fragments[0].placements[0].implementation_id =
                    "implementation/foreign".into()
            }
            _ => input.revision += 1,
        }
        let revision = state.revision().unwrap();
        begin_and_write(
            &mut state,
            &mut transfer,
            InputKind::FaceInteraction,
            &data::encode_interaction(&input).unwrap(),
        );
        assert_eq!(state.revision(), Ok(revision));
        let refused = state.execute(Request::Commit, &mut transfer);
        assert!(refused.is_err());
        if variant == 2 {
            assert_eq!(refused, Err(Refusal::StaleFace));
        }
        assert_eq!(state.revision(), Ok(revision));
        assert_eq!(
            state.execute(Request::Commit, &mut transfer),
            Err(Refusal::Order)
        );
        let (_, original) = fixture();
        commit_valid(&mut state, &mut transfer, &original);
    }
}
#[test]
fn transfer_boundaries_and_blob_ceiling_preserve_deferred_malformed_refusal() {
    let (mut state, mut transfer, input) = initialized();
    assert_eq!(
        state.execute(
            Request::Begin {
                kind: InputKind::FaceInteraction,
                total: 0
            },
            &mut transfer
        ),
        Err(Refusal::Order)
    );
    assert_eq!(
        state.execute(
            Request::Begin {
                kind: InputKind::FaceInteraction,
                total: MAX_BLOB_BYTES + 1
            },
            &mut transfer
        ),
        Err(Refusal::Order)
    );
    for length in [
        1,
        TRANSFER_BYTES as usize - 1,
        TRANSFER_BYTES as usize,
        TRANSFER_BYTES as usize + 1,
        MAX_BLOB_BYTES as usize,
    ] {
        begin_and_write(
            &mut state,
            &mut transfer,
            InputKind::FaceInteraction,
            &vec![0; length],
        );
        assert_eq!(state.revision(), Ok(input.revision));
        assert_eq!(
            state.execute(Request::Commit, &mut transfer),
            Err(Refusal::Malformed)
        );
        assert_eq!(
            state.execute(Request::Commit, &mut transfer),
            Err(Refusal::Order)
        );
    }
    commit_valid(&mut state, &mut transfer, &input);
}
