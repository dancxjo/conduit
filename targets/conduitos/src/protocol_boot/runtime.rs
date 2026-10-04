//! The containing machine owns the bounded loop around the admitted kernel.
use super::{owners::NativeOwners, request::ProtocolBootRequest};
use crate::{
    arch,
    protocol_source::{
        NativeProtocolPreparationLimits, PreparedProtocolBodyPlay, PreparedProtocolEntry,
        prepare_native_protocol,
    },
};
use alloc::{vec, vec::Vec};
use conduit_composite::KernelCompositeStatus;
use conduit_core::*;

type NativePlay = PreparedProtocolBodyPlay<
    crate::i2c_base::i801::I801Controller<arch::I801PortWindow>,
    arch::NativeMonotonicDeadlineClock,
>;

pub(super) struct Output {
    port: PortId,
    value: ValuePayload,
}

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
    let definition = play
        .kernel()
        .ok_or("protocol-kernel-unavailable")?
        .definition();
    if request.inputs.len() != definition.external_capability.inputs.len()
        || request.inputs.iter().any(|input| {
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

pub(super) fn execute(
    play: &mut NativePlay,
    outputs: &mut [Output],
    request: &ProtocolBootRequest,
) -> Result<(), &'static str> {
    // All allocations and Fore shape checks precede start. Ingress does not
    // dispatch physical effects; refusal cancels before the first Step.
    let definition = play
        .kernel()
        .ok_or("protocol-kernel-unavailable")?
        .definition();
    let mut inputs = Vec::with_capacity(request.inputs.len());
    for input in &request.inputs {
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
        for _ in 0..request.maximum_steps {
            let status = play.step().map_err(|_| "protocol-step-refused")?;
            for output in &mut *outputs {
                if let Some(sequence) = play
                    .output_into(&output.port, &mut output.value)
                    .map_err(|_| "protocol-output-refused")?
                {
                    // Bootstrap diagnostics retain the exact typed bytes. This
                    // does not reinterpret device data or manufacture a Face.
                    report_output(&output.port, &output.value)?;
                    play.complete_output(&output.port, sequence)
                        .map_err(|_| "protocol-output-close-refused")?;
                }
            }
            if status == KernelCompositeStatus::Complete {
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
