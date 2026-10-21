use super::*;
use crate::data::{InitManifest, InitSlot, Initial, Target};
use alloc::{vec, vec::Vec};
fn manifest(slots: [Option<InitSlot>; 4]) -> Command {
    Command::PrepareInit {
        manifest: InitManifest { slots },
    }
}
fn call(state: &mut State, command: &Command) -> Result<Reply, Refusal> {
    let input = data::encode_command(command)?;
    let mut transfer = vec![0; crate::memory::TRANSFER_BYTES];
    state.execute(
        Request::Begin {
            bytes: input.len() as u32,
        },
        &mut transfer,
    )?;
    for (index, chunk) in input.chunks(transfer.len()).enumerate() {
        transfer[..chunk.len()].copy_from_slice(chunk);
        state.execute(
            Request::Write {
                offset: (index * transfer.len()) as u32,
                bytes: chunk.len() as u32,
            },
            &mut transfer,
        )?;
    }
    if matches!(command, Command::PrepareTarget { .. }) {
        assert_eq!(
            state.execute(Request::Preflight, &mut transfer)?,
            Response::Ready
        );
    }
    let Response::Blob { bytes } = state.execute(Request::Commit, &mut transfer)? else {
        return Err(Refusal::Malformed);
    };
    let mut output = Vec::new();
    for offset in (0..bytes as usize).step_by(transfer.len()) {
        let Response::Chunk { bytes, .. } = state.execute(
            Request::Read {
                offset: offset as u32,
                maximum: transfer.len() as u32,
            },
            &mut transfer,
        )?
        else {
            return Err(Refusal::Malformed);
        };
        output.extend_from_slice(&transfer[..bytes as usize]);
    }
    data::decode_reply(&output)
}
fn terminal(state: &mut State) {
    assert!(state.models.is_none());
    assert!(state.preparing.is_none());
    assert!(state.projection.is_none());
    assert!(state.pending_target.is_none());
    assert!(state.refused);
    assert!(matches!(
        call(state, &Command::FinishInit),
        Err(Refusal::Order)
    ));
}
#[test]
fn finish_moves_prepared_controllers_only_after_complete_acknowledgement() {
    let mut state = State::default();
    assert!(matches!(
        call(
            &mut state,
            &manifest([Some(InitSlot::Tour), None, Some(InitSlot::Tour), None])
        ),
        Ok(Reply::Prepared)
    ));
    assert!(state.models.is_none());
    for slot in [0, 2] {
        assert!(matches!(
            call(&mut state, &Command::PrepareTour { slot }),
            Ok(Reply::Prepared)
        ));
        assert!(state.models.is_none());
    }
    assert!(matches!(
        call(&mut state, &Command::FinishInit),
        Ok(Reply::Initialized)
    ));
    assert!(state.preparing.is_none());
    assert_eq!(
        state.models.as_ref().unwrap().kind(0),
        Some(crate::data::Kind::Tour)
    );
    assert_eq!(
        state.models.as_ref().unwrap().kind(2),
        Some(crate::data::Kind::Tour)
    );
}
#[test]
fn missing_duplicate_out_of_order_and_early_use_are_terminal() {
    for case in 0..7 {
        let mut state = State::default();
        let declaration = manifest([Some(InitSlot::Tour), None, Some(InitSlot::Tour), None]);
        call(&mut state, &declaration).unwrap();
        if matches!(case, 1 | 2 | 5) {
            call(&mut state, &Command::PrepareTour { slot: 0 }).unwrap();
        }
        let invalid = match case {
            0 | 1 => Command::FinishInit,
            2 => Command::PrepareTour { slot: 0 },
            3 => Command::PrepareTour { slot: 2 },
            4 => declaration,
            5 => Command::Graph { slot: 0 },
            _ => Command::PrepareTour { slot: 4 },
        };
        assert!(
            matches!(call(&mut state, &invalid), Err(Refusal::Order)),
            "case {case}"
        );
        terminal(&mut state);
    }
}
#[test]
fn invalid_manifest_is_rejected_before_controller_construction() {
    for (targets, selected) in [(0, 0), (16, 0), (1, 1), (1, 4), (255, 255)] {
        let mut state = State::default();
        assert!(matches!(
            call(
                &mut state,
                &manifest([
                    Some(InitSlot::Patchbay { targets, selected }),
                    None,
                    None,
                    None
                ])
            ),
            Err(Refusal::Bounds)
        ));
        terminal(&mut state);
    }
    assert!(matches!(
        data::decode::<Command>(
            br#"{"PrepareInit":{"manifest":{"slots":[null,null,null,null,null]}}}"#
        ),
        Err(Refusal::Malformed)
    ));
    // The previous aggregate wire entrance is deliberately gone.
    assert!(matches!(
        data::decode::<Command>(br#"{"Init":{"applications":[null,null,null,null]}}"#),
        Err(Refusal::Malformed)
    ));
}
fn small_target() -> Target {
    #[derive(serde::Deserialize)]
    enum LegacyFixture {
        Init { applications: [Option<Initial>; 4] },
    }
    let LegacyFixture::Init { mut applications } =
        data::decode(include_bytes!("../../tests/fixtures/startup.catz")).unwrap();
    let Some(Initial::Patchbay { mut targets, .. }) = applications[0].take() else {
        panic!("fixture");
    };
    targets[1].take().unwrap()
}
#[test]
fn exact_target_coverage_and_selected_position_survive_staging() {
    let first = small_target();
    let mut selected = first.clone();
    selected.graph.plot_name = "exact selected target".into();
    let mut state = State::default();
    call(
        &mut state,
        &manifest([
            Some(InitSlot::Patchbay {
                targets: 0b1010,
                selected: 3,
            }),
            None,
            None,
            None,
        ]),
    )
    .unwrap();
    for (target, value) in [(1, first), (3, selected.clone())] {
        assert!(matches!(
            call(
                &mut state,
                &Command::PrepareTarget {
                    slot: 0,
                    target,
                    value
                }
            ),
            Ok(Reply::Prepared)
        ));
        assert!(state.models.is_none());
        assert!(state.pending_target.is_some());
        assert!(matches!(
            call(&mut state, &Command::FinishTarget { slot: 0, target }),
            Ok(Reply::Prepared)
        ));
        assert!(state.pending_target.is_none());
    }
    call(&mut state, &Command::FinishInit).unwrap();
    assert_eq!(
        state.models.as_ref().unwrap().graph(0),
        Some(&selected.graph)
    );
}
#[test]
fn wrong_target_order_kind_and_duplicate_fail_without_model_publication() {
    for case in 0..3 {
        let mut state = State::default();
        let target = small_target();
        call(
            &mut state,
            &manifest([
                Some(InitSlot::Patchbay {
                    targets: 0b1010,
                    selected: 1,
                }),
                None,
                None,
                None,
            ]),
        )
        .unwrap();
        if case == 2 {
            call(
                &mut state,
                &Command::PrepareTarget {
                    slot: 0,
                    target: 1,
                    value: target.clone(),
                },
            )
            .unwrap();
            call(&mut state, &Command::FinishTarget { slot: 0, target: 1 }).unwrap();
        }
        let invalid = if case == 1 {
            Command::PrepareTour { slot: 0 }
        } else {
            Command::PrepareTarget {
                slot: 0,
                target: if case == 0 { 3 } else { 1 },
                value: target,
            }
        };
        assert!(matches!(call(&mut state, &invalid), Err(Refusal::Order)));
        terminal(&mut state);
    }
}

#[test]
fn selected_unfilled_target_cannot_finish() {
    let mut state = State::default();
    call(
        &mut state,
        &manifest([
            Some(InitSlot::Patchbay {
                targets: 0b1010,
                selected: 3,
            }),
            None,
            None,
            None,
        ]),
    )
    .unwrap();
    call(
        &mut state,
        &Command::PrepareTarget {
            slot: 0,
            target: 1,
            value: small_target(),
        },
    )
    .unwrap();
    assert!(matches!(
        call(&mut state, &Command::FinishTarget { slot: 0, target: 1 }),
        Ok(Reply::Prepared)
    ));
    assert!(matches!(
        call(&mut state, &Command::FinishInit),
        Err(Refusal::Order)
    ));
    terminal(&mut state);
}
#[test]
fn projection_during_staging_and_repeat_finish_are_terminal() {
    let mut state = State::default();
    call(
        &mut state,
        &manifest([Some(InitSlot::Tour), None, None, None]),
    )
    .unwrap();
    let projection = Command::PrepareGraphProjection {
        graph: small_target().graph,
        scope: crate::graph_facts::Scope {
            host: conduit_core::HostId::from("host/staged-test"),
            boot: conduit_core::BootId::from("boot/staged-test"),
            body: {
                #[derive(serde::Deserialize)]
                enum Fixture {
                    Init { applications: [Option<Initial>; 4] },
                }
                let Fixture::Init { mut applications } =
                    data::decode(include_bytes!("../../tests/fixtures/startup.catz")).unwrap();
                let Some(Initial::Tutorial { evidence }) = applications[1].take() else {
                    panic!("fixture");
                };
                evidence.body_id
            },
            owner_revision: 1,
            graph_digest: [0; 32],
            plot_present: false,
        },
    };
    assert!(matches!(call(&mut state, &projection), Err(Refusal::Order)));
    terminal(&mut state);
    let mut state = State::default();
    call(&mut state, &manifest([None; 4])).unwrap();
    call(&mut state, &Command::FinishInit).unwrap();
    assert!(matches!(
        call(&mut state, &Command::FinishInit),
        Err(Refusal::Order)
    ));
    terminal(&mut state);
}

mod projection;

#[test]
fn target_is_private_until_matching_finish_and_interleaving_is_terminal() {
    let declaration = manifest([
        Some(InitSlot::Patchbay {
            targets: 0b0010,
            selected: 1,
        }),
        None,
        None,
        None,
    ]);
    let mut state = State::default();
    call(&mut state, &declaration).unwrap();
    call(
        &mut state,
        &Command::PrepareTarget {
            slot: 0,
            target: 1,
            value: small_target(),
        },
    )
    .unwrap();
    assert!(state.models.is_none());
    assert!(state.pending_target.is_some());
    assert!(matches!(
        call(&mut state, &Command::FinishTarget { slot: 0, target: 1 }),
        Ok(Reply::Prepared)
    ));
    assert!(state.pending_target.is_none());
    assert!(matches!(
        call(&mut state, &Command::FinishInit),
        Ok(Reply::Initialized)
    ));

    for bad in [
        Command::FinishTarget { slot: 0, target: 0 },
        Command::FinishInit,
        Command::PrepareTour { slot: 1 },
    ] {
        let mut state = State::default();
        call(&mut state, &declaration).unwrap();
        call(
            &mut state,
            &Command::PrepareTarget {
                slot: 0,
                target: 1,
                value: small_target(),
            },
        )
        .unwrap();
        assert!(matches!(call(&mut state, &bad), Err(Refusal::Order)));
        terminal(&mut state);
    }
}

#[test]
fn completed_preflight_rejects_later_writes_and_latches_refusal() {
    let mut state = State::default();
    call(
        &mut state,
        &manifest([
            Some(InitSlot::Patchbay {
                targets: 0b0010,
                selected: 1,
            }),
            None,
            None,
            None,
        ]),
    )
    .unwrap();
    let encoded = data::encode_command(&Command::PrepareTarget {
        slot: 0,
        target: 1,
        value: small_target(),
    })
    .unwrap();
    let mut transfer = vec![0; crate::memory::TRANSFER_BYTES];
    state
        .execute(
            Request::Begin {
                bytes: encoded.len() as u32,
            },
            &mut transfer,
        )
        .unwrap();
    for (index, chunk) in encoded.chunks(transfer.len()).enumerate() {
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
    assert_eq!(
        state.execute(Request::Preflight, &mut transfer),
        Ok(Response::Ready)
    );
    assert_eq!(
        state.execute(
            Request::Write {
                offset: encoded.len() as u32,
                bytes: 1,
            },
            &mut transfer
        ),
        Err(Refusal::Order)
    );
    terminal(&mut state);
}
