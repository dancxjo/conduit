//! The containing machine owns the bounded loop around the admitted kernel.
#[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
#[path = "runtime_network.rs"]
mod network;
#[cfg(target_arch = "x86_64")]
use super::{owners::NativeOwners, request::ProtocolBootRequest};
use crate::{
    arch,
    protocol_source::{PreparedProtocolBodyPlay, PreparedProtocolEntry},
};
use alloc::{vec, vec::Vec};
use conduit_composite::KernelCompositeStatus;
use conduit_core::*;

#[cfg(target_arch = "x86_64")]
type NativePlay = PreparedProtocolBodyPlay<
    crate::i2c_base::i801::I801Controller<arch::I801PortWindow>,
    arch::NativeMonotonicDeadlineClock,
>;

#[cfg(target_arch = "aarch64")]
type NativePlay = PreparedProtocolBodyPlay<
    super::absent_providers::AbsentI2c,
    super::absent_providers::AbsentClock,
>;

#[cfg(target_arch = "x86_64")]
use crate::protocol_source::{NativeProtocolPreparationLimits, prepare_native_protocol};

pub(super) struct Output {
    port: PortId,
    value: ValuePayload,
}

#[cfg(target_arch = "x86_64")]
pub(super) fn prepare(
    entry: PreparedProtocolEntry,
    request: &ProtocolBootRequest,
    host: HostId,
    boot: BootId,
    owners: NativeOwners,
) -> Result<(NativePlay, Vec<Output>), &'static str> {
    let session = super::body::admit(entry.resident(), &host, &boot)?;
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let play = prepare_native_protocol(
        entry,
        session,
        advertisement,
        owners,
        NativeProtocolPreparationLimits {
            body_play_sequence: 0,
            bus_work_units: 1,
            clock_work_units: crate::monotonic_clock::owner::MAXIMUM_POLL_STEPS,
        },
    )
    .map_err(|_| "protocol-plan-admission-refused")?;
    prepare_outputs(play, &request.inputs)
}

pub(super) fn prepare_usb(
    entry: PreparedProtocolEntry,
    request: &super::usb_request::UsbProtocolBootRequest,
    host: HostId,
    boot: BootId,
    owners: super::usb_owners::NativeUsbOwners,
) -> Result<(NativePlay, Vec<Output>), &'static str> {
    let session = super::body::admit(entry.resident(), &host, &boot)?;
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let play = match owners {
        super::usb_owners::NativeUsbOwners::Isochronous {
            ready,
            control_issuer,
            isochronous_issuer,
        } => crate::protocol_source::prepare_native_usb_isochronous_protocol(
            entry,
            session,
            advertisement,
            *ready,
            *control_issuer,
            *isochronous_issuer,
            0,
            1,
            request.additional_remote_sign_items,
        ),
        super::usb_owners::NativeUsbOwners::Duplex {
            ready,
            control_issuer,
            read_issuer,
            write_issuer,
            #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
            network,
        } => {
            #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
            let result = match network {
                Some(network) => crate::protocol_source::prepare_native_usb_duplex_with_network(
                    entry,
                    session,
                    advertisement,
                    *ready,
                    *control_issuer,
                    *read_issuer,
                    *write_issuer,
                    0,
                    1,
                    request.additional_remote_sign_items,
                    Some(crate::protocol_source::NativeEthernetPreparation {
                        ready: *network.ready,
                        issuer: *network.issuer,
                        random_seed: network.random_seed,
                        timestamp: smoltcp::time::Instant::ZERO,
                    }),
                ),
                None => crate::protocol_source::prepare_native_usb_duplex_protocol(
                    entry,
                    session,
                    advertisement,
                    *ready,
                    *control_issuer,
                    *read_issuer,
                    *write_issuer,
                    0,
                    1,
                    request.additional_remote_sign_items,
                ),
            };
            #[cfg(not(all(target_arch = "x86_64", feature = "native-owner-network")))]
            let result = crate::protocol_source::prepare_native_usb_duplex_protocol(
                entry,
                session,
                advertisement,
                *ready,
                *control_issuer,
                *read_issuer,
                *write_issuer,
                0,
                1,
                request.additional_remote_sign_items,
            );
            result
        }
        super::usb_owners::NativeUsbOwners::Control { ready, issuer } => {
            crate::protocol_source::prepare_native_usb_protocol_with_sign_storage(
                entry,
                session,
                advertisement,
                *ready,
                *issuer,
                0,
                1,
                request.additional_remote_sign_items,
            )
        }
        super::usb_owners::NativeUsbOwners::Protocol {
            ready,
            control_issuer,
            endpoint_issuer,
        } => crate::protocol_source::prepare_native_usb_endpoint_protocol(
            entry,
            session,
            advertisement,
            *ready,
            *control_issuer,
            *endpoint_issuer,
            0,
            1,
            request.additional_remote_sign_items,
        ),
    }
    .map_err(|refusal| {
        report_preparation_refusal(&refusal.reason);
        "protocol-usb-plan-admission-refused"
    })?;
    prepare_outputs(play, &request.inputs)
}

fn prepare_outputs(
    play: NativePlay,
    inputs: &[super::ProtocolBootInput],
) -> Result<(NativePlay, Vec<Output>), &'static str> {
    let definition = play
        .kernel()
        .ok_or("protocol-kernel-unavailable")?
        .definition();
    if inputs.len() != definition.external_capability.inputs.len()
        || inputs.iter().any(|input| {
            !definition
                .external_capability
                .inputs
                .iter()
                .any(|port| port.port_id.as_str() == input.port)
        })
    {
        return Err("protocol-input-workset-mismatch");
    }
    let mut outputs = Vec::with_capacity(definition.external_capability.outputs.len());
    for port in &definition.external_capability.outputs {
        let maximum = definition
            .internal_plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.fore_ports)
            .filter(|boundary| {
                boundary.front_port_id == port.port_id
                    && boundary.direction == PortDirection::Output
                    && boundary.track == ConnectionTrack::Payload
            })
            .map(|boundary| boundary.byte_capacity)
            .min()
            .ok_or("protocol-output-boundary-missing")?;
        if maximum > crate::protocol_source::MAXIMUM_PROTOCOL_FORE_BYTES {
            return Err("protocol-output-envelope-unsupported");
        }
        outputs.push(Output {
            port: port.port_id.clone(),
            value: ValuePayload {
                value_kind: port.value_kind.clone(),
                encoded: Vec::with_capacity(maximum as usize),
            },
        });
    }
    report_plan(&definition.internal_plan)?;
    Ok((play, outputs))
}

#[cfg(target_arch = "x86_64")]
pub(super) fn execute(
    play: &mut NativePlay,
    outputs: &mut [Output],
    request: &ProtocolBootRequest,
) -> Result<(), &'static str> {
    execute_workload(
        play,
        outputs,
        &request.inputs,
        request.maximum_steps,
        #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
        None,
    )
}

pub(super) fn execute_usb(
    play: &mut NativePlay,
    outputs: &mut [Output],
    request: &super::usb_request::UsbProtocolBootRequest,
) -> Result<(), &'static str> {
    #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
    let network_deadline = network::prepare(request)?;
    execute_workload(
        play,
        outputs,
        &request.inputs,
        request.maximum_steps,
        #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
        network_deadline,
    )
}

fn execute_workload(
    play: &mut NativePlay,
    outputs: &mut [Output],
    authored_inputs: &[super::ProtocolBootInput],
    maximum_steps: u32,
    #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
    mut network_deadline: Option<network::RuntimeNetwork>,
) -> Result<(), &'static str> {
    // All allocations and Fore shape checks precede start. Ingress does not
    // dispatch physical effects; refusal cancels before the first Step.
    let definition = play
        .kernel()
        .ok_or("protocol-kernel-unavailable")?
        .definition();
    let mut inputs = Vec::with_capacity(authored_inputs.len());
    for input in authored_inputs {
        let port = definition
            .external_capability
            .inputs
            .iter()
            .find(|port| port.port_id.as_str() == input.port)
            .ok_or("protocol-input-port-missing")?;
        inputs.push((
            port.port_id.clone(),
            ValuePayload {
                value_kind: port.value_kind.clone(),
                encoded: input.canonical_bytes.clone(),
            },
        ));
    }
    play.start().map_err(|_| "protocol-start-refused")?;
    let result = (|| {
        report_play(play)?;
        for (port, value) in &inputs {
            play.admit_input(port, 0, value)
                .map_err(|_| "protocol-input-refused")?;
            play.close_input(port)
                .map_err(|_| "protocol-input-close-refused")?;
        }
        for _ in 0..maximum_steps {
            #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
            if let Some(network) = network_deadline.as_mut() {
                network.poll(play)?;
            }
            let status = play.step().map_err(|error| {
                if let Some(identity) = play.session().realization().and_then(|r| r.play.as_ref()) {
                    match super::step_failure::format(&error, identity) {
                        Ok(Some(report)) => {
                            let _ = arch::append_boot_diagnostic(report.as_bytes());
                        }
                        Err(_) => {
                            let _ = arch::append_boot_diagnostic(
                                b"CONDUIT_PROTOCOL_STEP_FAILURE_UNRETAINED envelope-exceeded\n",
                            );
                        }
                        Ok(None) => {}
                    }
                }
                "protocol-step-refused"
            })?;
            for output in &mut *outputs {
                if let Some(sequence) = play
                    .output_into(&output.port, &mut output.value)
                    .map_err(|_| "protocol-output-refused")?
                {
                    // Bootstrap diagnostics retain the exact typed bytes. This
                    // does not reinterpret device data or manufacture a Face.
                    report_output(&output.port, &output.value)?;
                    play.complete_output(&output.port, sequence)
                        .map_err(|error| output_close_refusal(&error))?;
                }
            }
            if status == KernelCompositeStatus::Complete {
                #[cfg(all(target_arch = "x86_64", feature = "native-owner-network"))]
                if network_deadline
                    .as_ref()
                    .is_some_and(|network| !network.complete())
                {
                    return Err("protocol-network-not-complete");
                }
                return Ok(());
            }
            if status == KernelCompositeStatus::Cancelled {
                return Err("protocol-unexpected-cancellation");
            }
            if play.has_pending_clock() {
                arch::space_protocol_clock_poll()?;
            }
        }
        Err("protocol-step-budget-exhausted")
    })();
    play.cancel().map_err(|_| "protocol-retirement-refused")?;
    result
}

fn report_output(port: &PortId, value: &ValuePayload) -> Result<(), &'static str> {
    use core::fmt::Write;
    let mut header = crate::sign_format::FixedText::new();
    writeln!(
        header,
        "CONDUIT_PROTOCOL_OUTPUT port={} kind={} bytes={}",
        port.as_str(),
        value.value_kind.as_str(),
        value.encoded.len()
    )
    .map_err(|_| "protocol-diagnostic-envelope-exceeded")?;
    arch::append_boot_diagnostic(header.as_bytes())
        .map_err(|_| "protocol-diagnostic-unavailable")?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in &value.encoded {
        arch::append_boot_diagnostic(&[HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]])
            .map_err(|_| "protocol-diagnostic-unavailable")?;
    }
    arch::append_boot_diagnostic(b"\n").map_err(|_| "protocol-diagnostic-unavailable")?;
    Ok(())
}

fn report_plan(plan: &Plan) -> Result<(), &'static str> {
    use core::fmt::Write;
    let mut text = crate::sign_format::FixedText::new();
    writeln!(
        text,
        "CONDUIT_PROTOCOL_PLAN source={} checked={} expanded={} plan={}",
        plan.source_document_id.as_str(),
        plan.checked_plot_id.as_str(),
        plan.expanded_plot_id.as_str(),
        plan.plan_id.as_str()
    )
    .map_err(|_| "protocol-plan-sign-envelope-exceeded")?;
    arch::append_boot_diagnostic(text.as_bytes()).map_err(|_| "protocol-diagnostic-unavailable")?;
    for fragment in &plan.fragments {
        for gear in &fragment.placements {
            let Some(base) = &gear.base else {
                continue;
            };
            let mut selected = crate::sign_format::FixedText::new();
            writeln!(selected,
                "CONDUIT_PROTOCOL_BASE plan={} placement={} host={} boot={} base={} provider={} generation={} artifact={}",
                plan.plan_id.as_str(), gear.placement_id.as_str(), fragment.host_id.as_str(),
                fragment.boot_id.as_str(), base.base_id.as_str(), base.provider_instance_id.as_str(),
                base.provider_generation, gear.artifact_id.as_str())
                .map_err(|_| "protocol-base-sign-envelope-exceeded")?;
            arch::append_boot_diagnostic(selected.as_bytes())
                .map_err(|_| "protocol-diagnostic-unavailable")?;
            for resource in &gear.resources {
                let mut selected = crate::sign_format::FixedText::new();
                writeln!(
                    selected,
                    "CONDUIT_PROTOCOL_RESOURCE plan={} placement={} pool={} class={} units={}",
                    plan.plan_id.as_str(),
                    gear.placement_id.as_str(),
                    resource.pool_id.as_str(),
                    resource.class_id.as_str(),
                    resource.units
                )
                .map_err(|_| "protocol-resource-sign-envelope-exceeded")?;
                arch::append_boot_diagnostic(selected.as_bytes())
                    .map_err(|_| "protocol-diagnostic-unavailable")?;
            }
        }
    }
    Ok(())
}

fn report_play(play: &NativePlay) -> Result<(), &'static str> {
    use core::fmt::Write;
    let identity = play
        .session()
        .realization()
        .and_then(|current| current.play.as_ref())
        .ok_or("protocol-current-play-missing")?;
    let mut text = crate::sign_format::FixedText::new();
    writeln!(
        text,
        "CONDUIT_PROTOCOL_PLAY body={} wake={} plan={} play={}",
        identity.body_id.as_str(),
        identity.wake_id.as_str(),
        identity.plan_id.as_str(),
        identity.active_play_id.as_str()
    )
    .map_err(|_| "protocol-play-sign-envelope-exceeded")?;
    arch::append_boot_diagnostic(text.as_bytes()).map_err(|_| "protocol-diagnostic-unavailable")?;
    Ok(())
}

// Keep the exact finite-storage refusal observable without allocating in Play.
fn output_close_refusal(error: &crate::protocol_source::ProtocolBodyRefusal) -> &'static str {
    use conduit_composite::{ChildExecutionError, KernelCompositeError};
    use conduit_kernel::{SignError, scheduler::SchedulerError};
    let crate::protocol_source::ProtocolBodyRefusal::Call(
        crate::protocol_host_calls::ProtocolCallRefusal::Kernel(KernelCompositeError::Execution {
            reason: ChildExecutionError::Scheduler(SchedulerError::Sign(reason)),
            ..
        }),
    ) = error
    else {
        return "protocol-output-close-refused";
    };
    match reason {
        SignError::InvalidBudget => "protocol-output-sign-invalid-budget",
        SignError::ItemCapacityExceeded => "protocol-output-sign-items-exhausted",
        SignError::ByteCapacityExceeded => "protocol-output-sign-bytes-exhausted",
        SignError::RemoteItemCapacityExceeded => "protocol-output-sign-remote-items-exhausted",
        SignError::RemoteByteCapacityExceeded => "protocol-output-sign-remote-bytes-exhausted",
        SignError::SequenceOverflow => "protocol-output-sign-sequence-overflow",
    }
}

#[cfg(test)]
mod reuse_tests;

fn report_preparation_refusal(reason: &impl core::fmt::Debug) {
    use core::fmt::Write;
    let mut report = crate::sign_format::FixedText::new();
    if writeln!(report, "CONDUIT_PROTOCOL_PREPARATION_REFUSAL {reason:?}").is_ok() {
        let _ = arch::append_boot_diagnostic(report.as_bytes());
    }
}
