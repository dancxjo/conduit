//! Containing-machine loop for one checked, finite display diagnostic.
//! All display commands, D/C transitions and delays are authored Source.
use super::{frame_chunks::PreparedFrameChunks, minipitft_owners};
use crate::{
    arch,
    display::rgb565::{Orientation, Rgb565Raster},
    protocol_play::NoI2cProvider,
    protocol_source::{PreparedProtocolBodyPlay, PreparedProtocolEntry, prepare_native_peripheral},
};
use alloc::{vec, vec::Vec};
use conduit_core::*;
use conduit_face::FaceBasis;
use conduit_kernel::scheduler::RemoteIngressOutcome;

type NativePlay = PreparedProtocolBodyPlay<
    NoI2cProvider,
    arch::deadline::DeadlineClock<arch::deadline::SystemTimer>,
>;
pub(super) struct RetainedDisplay {
    // Success retains the actuators. Dropping this record invokes safe-off.
    pub(super) play: NativePlay,
    pub(super) buttons: minipitft_owners::RemainingButtons,
}
struct Output {
    port: PortDescriptor,
    value: ValuePayload,
}
fn output(entry: &PreparedProtocolEntry, name: &str) -> Result<Output, &'static str> {
    let port = entry
        .expanded()
        .front
        .outputs()
        .iter()
        .find(|port| port.port_id.as_str() == name)
        .ok_or("minipitft-output-missing")?
        .clone();
    Ok(Output {
        value: ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        },
        port,
    })
}
fn digital_gear(entry: &PreparedProtocolEntry, name: &str) -> Result<GearId, &'static str> {
    let mut matches = entry.expanded().expanded.gears.iter().filter(|gear| {
        gear.kind_id.as_str() == "machine/digital/access"
            && gear.gear_id.as_str().rsplit('/').next() == Some(name)
    });
    let gear = matches.next().ok_or("minipitft-contact-gear-missing")?;
    if matches.next().is_some() {
        return Err("minipitft-contact-gear-ambiguous");
    }
    Ok(gear.gear_id.clone())
}

/// # Safety
/// The image's native Root exclusively owns the selected B+ attachment,
/// firmware handoff, calibrated clocks and electrical configuration.
pub(super) unsafe fn run(
    entry: PreparedProtocolEntry,
    host: HostId,
    boot: BootId,
    artifact: &str,
    stage: fn(u8),
) -> Result<RetainedDisplay, &'static str> {
    let gears = [
        digital_gear(&entry, "backlight")?,
        digital_gear(&entry, "command-data")?,
    ];
    let input = entry
        .expanded()
        .front
        .inputs()
        .iter()
        .find(|port| port.port_id.as_str() == "chunks")
        .ok_or("minipitft-chunk-input-missing")?
        .clone();
    let schema = entry
        .input_schema(&input.port_id)
        .ok_or("minipitft-chunk-schema-missing")?;
    let mut requested = output(&entry, "requested")?;
    let mut completed = output(&entry, "completed")?;
    let mut refused = output(&entry, "refused")?;
    let mut chunks = PreparedFrameChunks::new(&schema, 240, 240, 256, Orientation::Rotate180)
        .map_err(|_| "minipitft-chunk-preparation-refused")?;
    let mut input_value = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let session = super::body::admit(entry.resident(), &host, &boot)?;
    let (mut owners, buttons) = unsafe { minipitft_owners::admit(&host, &boot, artifact, gears) }?;
    owners.sign_storage = super::minipitft_source::sign_storage(chunks.count(), 4)?;
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
    let mut play = prepare_native_peripheral(entry, session, advertisement, owners, 0)
        .map_err(|_| "minipitft-native-plan-refused")?;
    stage(4);
    let plan = &play
        .kernel()
        .ok_or("minipitft-kernel-missing")?
        .definition()
        .internal_plan;
    let basis = FaceBasis {
        body_id: Some(play.session().evidence().body_id.clone()),
        wake_id: None,
        source_document_id: Some(plan.source_document_id.clone()),
        checked_plot_id: Some(plan.checked_plot_id.clone()),
        expanded_plot_id: Some(plan.expanded_plot_id.clone()),
        plan_id: Some(plan.plan_id.clone()),
        active_play_id: None,
        sign_ids: vec![],
    };
    // This is explicitly a diagnostic CPU frame; it claims no Mask/Show receipt.
    let pixels = super::minipitft_face::render(basis, 1)
        .map_err(|_| "minipitft-diagnostic-frame-refused")?;
    let raster = Rgb565Raster::new(&pixels, 240, 240).map_err(|_| "minipitft-raster-refused")?;
    stage(5);
    play.start().map_err(|_| "minipitft-play-start-refused")?;
    let result = drive(
        &mut play,
        &raster,
        &mut chunks,
        &input,
        &mut input_value,
        &mut requested,
        &mut completed,
        &mut refused,
    );
    if let Err(error) = result {
        let _ = play.cancel();
        return Err(error);
    }
    Ok(RetainedDisplay { play, buttons })
}

#[allow(clippy::too_many_arguments)]
fn drive(
    play: &mut NativePlay,
    raster: &Rgb565Raster<'_>,
    chunks: &mut PreparedFrameChunks,
    input: &PortDescriptor,
    input_value: &mut ValuePayload,
    requested: &mut Output,
    completed: &mut Output,
    refused: &mut Output,
) -> Result<(), &'static str> {
    let start = arch::read_counter() as u32;
    let mut next = 0;
    for _ in 0..2_000_000 {
        if (arch::read_counter() as u32).wrapping_sub(start) > 180_000_000 {
            return Err("minipitft-play-time-exhausted");
        }
        play.step().map_err(|_| "minipitft-kernel-step-refused")?;
        if let Some(sequence) = play
            .output_into(&requested.port.port_id, &mut requested.value)
            .map_err(|_| "minipitft-request-output-refused")?
        {
            let value = validate_canonical_structured_value(&requested.value.encoded)
                .map_err(|_| "minipitft-request-malformed")?;
            let index = value
                .record_field("index")
                .map_err(|_| "minipitft-request-malformed")?
                .ok_or("minipitft-request-index-missing")?
                .primitive_bytes("value/u64")
                .map_err(|_| "minipitft-request-index-malformed")?;
            let index = u64::from_le_bytes(
                index
                    .try_into()
                    .map_err(|_| "minipitft-request-index-malformed")?,
            );
            if index != next {
                return Err("minipitft-request-index-stale");
            }
            let encoded = chunks
                .encode(raster, index)
                .map_err(|_| "minipitft-chunk-encoding-refused")?;
            input_value.encoded.clear();
            input_value.encoded.extend_from_slice(encoded);
            if matches!(
                play.admit_input(&input.port_id, next, input_value)
                    .map_err(|_| "minipitft-chunk-input-refused")?,
                RemoteIngressOutcome::Accepted { .. }
            ) {
                play.complete_output(&requested.port.port_id, sequence)
                    .map_err(|_| "minipitft-request-ack-refused")?;
                next += 1;
                if next == chunks.count() {
                    play.close_input(&input.port_id)
                        .map_err(|_| "minipitft-chunk-close-refused")?;
                }
            }
        }
        if let Some(sequence) = play
            .output_into(&refused.port.port_id, &mut refused.value)
            .map_err(|_| "minipitft-refusal-output-refused")?
        {
            play.complete_output(&refused.port.port_id, sequence)
                .map_err(|_| "minipitft-refusal-ack-refused")?;
            return Err("minipitft-source-refused");
        }
        if let Some(sequence) = play
            .output_into(&completed.port.port_id, &mut completed.value)
            .map_err(|_| "minipitft-completion-output-refused")?
        {
            let count = u64::from_le_bytes(
                completed
                    .value
                    .encoded
                    .as_slice()
                    .try_into()
                    .map_err(|_| "minipitft-completion-malformed")?,
            );
            if count != chunks.count() || next != count {
                return Err("minipitft-completion-inexact");
            }
            play.complete_output(&completed.port.port_id, sequence)
                .map_err(|_| "minipitft-completion-ack-refused")?;
            return Ok(());
        }
        // Clock providers observe once per quantum; the containing machine
        // spaces pending polls by1ms, inside the provider's finite4096 budget.
        if play.has_pending_clock() {
            let start = arch::read_counter() as u32;
            let mut elapsed = false;
            for _ in 0..1_000_000 {
                if (arch::read_counter() as u32).wrapping_sub(start) >= 1000 {
                    elapsed = true;
                    break;
                }
                core::hint::spin_loop();
            }
            if !elapsed {
                return Err("minipitft-pacing-clock-unavailable");
            }
        }
    }
    Err("minipitft-play-work-exhausted")
}
