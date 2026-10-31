//! Complete linked descriptors retain historical wire and terminal laws.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/sealed_state.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn historical_terminal_ports(
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
    profile: &conduit_core::TerminalTransductionProfile,
) -> Result<(), KindValidationError> {
    use conduit_core::{
        AbnormalTerminalTransduction, CancellationTransduction, NormalCloseTransduction,
    };
    let input = inputs
        .iter()
        .find(|port| port.port_id == profile.input_port_id)
        .ok_or(KindValidationError::UnknownTerminalTransductionInput)?;
    let output = outputs
        .iter()
        .find(|port| port.port_id == profile.output_port_id)
        .ok_or(KindValidationError::UnknownTerminalTransductionOutput)?;
    if !matches!(&profile.normal_close, NormalCloseTransduction::NotAccepted)
        && (!matches!(
            input.temporal,
            conduit_core::PortTemporal::Flow { closes: true }
        ) || !matches!(
            output.temporal,
            conduit_core::PortTemporal::Flow { closes: true }
        ))
    {
        return Err(KindValidationError::TerminalCloseModalityMismatch);
    }
    if !matches!(&profile.abnormal, AbnormalTerminalTransduction::NotAccepted)
        && input.abnormal_kind.is_none()
    {
        return Err(KindValidationError::TerminalAbnormalKindMismatch);
    }
    if matches!(
        &profile.abnormal,
        AbnormalTerminalTransduction::PropagateAfterDrain
            | AbnormalTerminalTransduction::FinalizeThenPropagate(_)
    ) && input.abnormal_kind != output.abnormal_kind
    {
        return Err(KindValidationError::TerminalAbnormalKindMismatch);
    }
    let close_bound = match &profile.normal_close {
        NormalCloseTransduction::FlushThenPropagate(bound)
        | NormalCloseTransduction::FlushThenPropagateWhenAllClose(bound) => Some(bound),
        _ => None,
    };
    let abnormal_bound = match &profile.abnormal {
        AbnormalTerminalTransduction::FinalizeThenPropagate(bound) => Some(bound),
        _ => None,
    };
    if close_bound
        .into_iter()
        .chain(abnormal_bound)
        .any(|bound| bound.maximum_items == 0 || bound.maximum_bytes == 0)
    {
        return Err(KindValidationError::EmptyTerminalEmissionBound);
    }
    let law_is_empty = match &profile.normal_close {
        NormalCloseTransduction::DomainSpecific { law } => law.as_str().is_empty(),
        _ => false,
    } || match &profile.abnormal {
        AbnormalTerminalTransduction::DomainSpecific { law } => law.as_str().is_empty(),
        _ => false,
    } || match &profile.cancellation {
        CancellationTransduction::DomainSpecific { law } => law.as_str().is_empty(),
        _ => false,
    };
    if law_is_empty {
        return Err(KindValidationError::EmptyTerminalLawIdentity);
    }
    if let CancellationTransduction::Request { disposition_kind } = &profile.cancellation {
        let cancellation_inputs = inputs
            .iter()
            .filter(|port| port.value_kind.as_str() == conduit_core::CANCELLATION_REQUEST_INFO_ID)
            .count();
        if cancellation_inputs != 1 {
            return Err(KindValidationError::CancellationControlMismatch);
        }
        if output.abnormal_kind.as_ref() != Some(disposition_kind) {
            return Err(KindValidationError::CancellationDispositionMismatch);
        }
    }
    Ok(())
}

#[test]
fn linked_ports_and_calls_hash_without_heap_and_preserve_every_field() {
    for temporal in [
        PortTemporal::Value,
        PortTemporal::Flow { closes: false },
        PortTemporal::Flow { closes: true },
        PortTemporal::Current,
    ] {
        for direction in [PortDirection::Input, PortDirection::Output] {
            for optional in [false, true] {
                let mut f = fixture::fragment();
                let gear = &mut f.placements[0];
                for p in gear.inputs.iter_mut().chain(&mut gear.outputs) {
                    p.temporal = temporal;
                    p.direction = direction;
                    p.abnormal_kind = optional.then(|| "fault\0é".into());
                }
                f.placements[0].host_calls = vec![HostCallRequirement {
                    contract_id: "call\0é".into(),
                    target_kind: optional.then(|| "target\0é".into()),
                    maximum_in_flight: u16::MAX,
                    maximum_input_bytes: u32::MAX,
                    maximum_output_bytes: 0,
                }];
                let gear = &f.placements[0];
                let inputs: Vec<_> = gear.inputs.iter().map(Into::into).collect();
                let outputs: Vec<_> = gear.outputs.iter().map(Into::into).collect();
                let calls: Vec<_> = gear.host_calls.iter().map(Into::into).collect();
                let mut view = PlannedGearRef::from(gear);
                view.inputs = PortDescriptorRefs::Borrowed(&inputs);
                view.outputs = PortDescriptorRefs::Borrowed(&outputs);
                view.host_calls = HostCallRequirementRefs::Borrowed(&calls);
                let (builder, cost) = allocation_probe::observe(|| {
                    let mut b = FragmentFingerprintBuilder::new_ref((&f).into(), 1);
                    b.push_placement_ref(view).unwrap();
                    b
                });
                assert_eq!((cost.allocations, cost.reallocations), (0, 0));
                assert_eq!(
                    builder.finish_ref((&f).into()).unwrap(),
                    compute_fragment_id(&f)
                );
                assert_eq!(view.inputs.get(inputs.len()), None);
            }
        }
    }
}
fn close(index: usize) -> NormalCloseTransduction {
    use NormalCloseTransduction::*;
    match index {
        0 => NotAccepted,
        1 => PropagateAfterDrain,
        2 => Consume,
        3 => FlushThenPropagate(FiniteTerminalEmission {
            maximum_items: u16::MAX,
            maximum_bytes: u32::MAX,
        }),
        4 => PropagateWhenAllClose,
        5 => FlushThenPropagateWhenAllClose(FiniteTerminalEmission {
            maximum_items: 0,
            maximum_bytes: 1,
        }),
        6 => DomainSpecific { law: "".into() },
        _ => DomainSpecific { law: "law".into() },
    }
}
fn abnormal(index: usize) -> AbnormalTerminalTransduction {
    use AbnormalTerminalTransduction::*;
    match index {
        0 => NotAccepted,
        1 => PropagateAfterDrain,
        2 => Recover,
        3 => FinalizeThenPropagate(FiniteTerminalEmission {
            maximum_items: u16::MAX,
            maximum_bytes: u32::MAX,
        }),
        4 => FinalizeThenPropagate(FiniteTerminalEmission {
            maximum_items: 1,
            maximum_bytes: 0,
        }),
        5 => DomainSpecific { law: "".into() },
        _ => DomainSpecific { law: "law".into() },
    }
}
fn cancel(index: usize) -> CancellationTransduction {
    use CancellationTransduction::*;
    match index {
        0 => NotCancellable,
        1 => Request {
            disposition_kind: "cancelled".into(),
        },
        2 => DomainSpecific { law: "".into() },
        _ => DomainSpecific { law: "law".into() },
    }
}
#[test]
fn complete_borrowed_terminal_validation_matches_historical_predicates() {
    for close_index in 0..8 {
        for abnormal_index in 0..7 {
            for cancel_index in 0..4 {
                for (mode, temporal) in [
                    PortTemporal::Value,
                    PortTemporal::Flow { closes: false },
                    PortTemporal::Flow { closes: true },
                    PortTemporal::Current,
                ]
                .into_iter()
                .enumerate()
                {
                    for flags in 0..16 {
                        let mut f = fixture::fragment();
                        f.states.clear();
                        let g = &mut f.placements[0];
                        g.inputs[0].temporal = temporal;
                        g.outputs[0].temporal = temporal;
                        g.inputs[0].abnormal_kind = (flags & 4 != 0).then(|| {
                            if mode % 2 == 0 {
                                "cancelled".into()
                            } else {
                                "fault".into()
                            }
                        });
                        g.outputs[0].abnormal_kind = (flags & 8 != 0).then(|| "cancelled".into());
                        for i in 0..mode % 3 {
                            g.inputs.push(PortDescriptor {
                                port_id: format!("cancel/{i}").into(),
                                value_kind: CANCELLATION_REQUEST_INFO_ID.into(),
                                direction: PortDirection::Input,
                                temporal: PortTemporal::Value,
                                abnormal_kind: None,
                            });
                        }
                        let profile = TerminalTransductionProfile {
                            input_port_id: if flags & 1 == 0 {
                                "next".into()
                            } else {
                                "missing".into()
                            },
                            output_port_id: if flags & 2 == 0 {
                                "current".into()
                            } else {
                                "missing".into()
                            },
                            normal_close: close(close_index),
                            abnormal: abnormal(abnormal_index),
                            cancellation: cancel(cancel_index),
                        };
                        let expected =
                            historical_terminal_ports(&g.inputs, &g.outputs, &profile).is_ok();
                        g.terminal_transductions = vec![profile];
                        let p = fixture::seal(f);
                        let f = &p.fragments[0];
                        let inputs: Vec<_> =
                            f.placements[0].inputs.iter().map(Into::into).collect();
                        let outputs: Vec<_> =
                            f.placements[0].outputs.iter().map(Into::into).collect();
                        let mut gear = PlannedGearRef::from(&f.placements[0]);
                        gear.inputs = PortDescriptorRefs::Borrowed(&inputs);
                        gear.outputs = PortDescriptorRefs::Borrowed(&outputs);
                        let gears = [gear];
                        let mut view = FlatPlanRef::from_plan(&p).unwrap();
                        view.placements = PlannedGearRefs::Borrowed(&gears);
                        assert_eq!(
                            view.verify().is_ok(),
                            expected,
                            "{close_index}/{abnormal_index}/{cancel_index}/{mode}/{flags}"
                        );
                    }
                }
            }
        }
    }
}
