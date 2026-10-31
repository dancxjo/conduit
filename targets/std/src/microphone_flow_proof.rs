//! Bounded physical/provider proof through an authored, admitted PCM output Fore.
use crate::hosted_microphone::{capture_flow::MicrophoneFlowLimits, AlsaMicrophoneAdapter};
use crate::{
    ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, StdHostComposition, StdHostConfig,
};
use conduit_core::{
    BaseImplementationId, ConnectionTrack, ObservationKind, PortDirection, TerminalDisposition,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::Instant};

#[derive(Debug, serde::Serialize)]
pub struct MicrophoneFlowProofReceipt {
    pub host_id: String,
    pub boot_id: String,
    pub plan_id: String,
    pub play_id: String,
    pub implementation_id: String,
    pub executable_sha256: String,
    pub card_id: String,
    pub device: u16,
    pub total_frames: u32,
    pub blocks: u16,
    pub capture_clock: u64,
    pub pcm_sha256: String,
    /// Consumer arrival latency, not an acquisition timestamp.
    pub first_delivery_microseconds: u64,
    pub elapsed_microseconds: u64,
}
struct Collector {
    started: Instant,
    first: Option<u64>,
    clock: Option<u64>,
    frames: u32,
    blocks: u16,
    block_frames: u16,
    total_frames: u32,
    hash: Sha256,
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        let (header, samples) = conduit_audio::PcmFrameHeader::decode_frame(&output.bytes)
            .map_err(|error| format!("PCM Fore frame: {error:?}"))?;
        let expected = u32::from(self.block_frames).min(self.total_frames - self.frames);
        if header.start_frame() != u64::from(self.frames)
            || u32::from(header.frame_count()) != expected
            || self.clock.is_some_and(|clock| clock != header.clock_id())
            || output.sequence != u64::from(self.blocks)
        {
            return Err("PCM Fore clock, sequence, offset or exact tail differs".into());
        }
        self.clock = Some(header.clock_id());
        self.first
            .get_or_insert(self.started.elapsed().as_micros() as u64);
        self.hash.update(samples);
        self.frames += expected;
        self.blocks += 1;
        Ok(())
    }
}
pub fn run(
    config: StdHostConfig,
    composition: StdHostComposition,
    microphone: AlsaMicrophoneAdapter,
    block_frames: u16,
) -> Result<MicrophoneFlowProofReceipt, Box<dyn std::error::Error>> {
    let host_id = config.host_id.as_str().to_owned();
    let boot_id = config.boot_id.as_str().to_owned();
    let milliseconds = microphone.limits().capture_milliseconds;
    let limits = MicrophoneFlowLimits {
        total_frames: milliseconds * 16,
        block_frames,
    };
    let blocks = limits.block_count()?;
    let implementation_id = microphone
        .flow_offer()
        .implementation
        .implementation_id
        .as_str()
        .to_owned();
    let executable_sha256 = microphone.executable_sha256().to_owned();
    let card_id = microphone.observation().card_id.clone();
    let device = microphone.observation().device;
    let mut host = StdHost::new_with_microphone(config, composition, microphone)?;
    let mut profile = conduit_plot::ProfileCatalog::new();
    let mut startup = conduit_plot::StartupCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_audio_capture_finite_catalog(&mut startup, &mut profile)?;
    let source = format!("plot microphone-flow-proof (\n audio: audio/pcm-frames@1...| >>\n) {{\n microphone: audio/capture-finite(capture-millis = {milliseconds}, block-frames = {block_frames})\n \"capture\" >> microphone.request\n microphone.audio >> audio\n}}\n");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .map_err(|error| format!("{} {}", error.code, error.message))?;
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "microphone-flow-proof",
        &profile,
    )
    .map_err(|error| format!("{} {}", error.code, error.message))?;
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded.expanded, &hosts)?;
    let empty = BTreeMap::new();
    let boundaries = BTreeMap::from([(
        conduit_planner::ForeBoundaryKey {
            direction: PortDirection::Output,
            front_port_id: conduit_core::port_id("audio"),
            track: ConnectionTrack::Payload,
        },
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: conduit_semantic_catalog::AUDIO_CAPTURE_FINITE_BLOCK_BYTES,
        },
    )]);
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 16,
            authority_grants: &[
                host.microphone_flow_authority_grant("grant/microphone-flow-proof")?
            ],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )?;
    let plan_id = plan.plan_id.as_str().to_owned();
    let fragment = plan
        .fragments
        .into_iter()
        .next()
        .ok_or("missing microphone fragment")?;
    let mut collector = Collector {
        started: Instant::now(),
        first: None,
        clock: None,
        frames: 0,
        blocks: 0,
        block_frames,
        total_frames: limits.total_frames,
        hash: Sha256::new(),
    };
    let report = host.run_external_plot_to(
        fragment,
        &[],
        &mut collector,
        &mut Vec::new(),
        &mut crate::ThreadTimer,
    )?;
    if collector.frames != limits.total_frames
        || collector.blocks != blocks
        || !matches!(
            report.observations.last().map(|event| &event.kind),
            Some(ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            })
        )
    {
        return Err("finite microphone Plan did not deliver its exact completed capture".into());
    }
    let kernel = report
        .kernel
        .ok_or("missing finite capture kernel receipt")?;
    if kernel.value_allocation_capacity_before != kernel.value_allocation_capacity_after {
        return Err("finite capture changed admitted value capacity during Play".into());
    }
    let play_id = kernel.active_play_id.as_str().to_owned();
    Ok(MicrophoneFlowProofReceipt {
        host_id,
        boot_id,
        plan_id,
        play_id,
        implementation_id,
        executable_sha256,
        card_id,
        device,
        total_frames: collector.frames,
        blocks: collector.blocks,
        capture_clock: collector.clock.ok_or("missing capture clock")?,
        pcm_sha256: format!("{:x}", collector.hash.finalize()),
        first_delivery_microseconds: collector.first.ok_or("missing first block")?,
        elapsed_microseconds: collector.started.elapsed().as_micros() as u64,
    })
}
