use conduit_audio::*;
use conduit_core::{BaseImplementationId, ConnectionTrack, PortDirection};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
#[derive(Default)]
struct Output(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Output {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value.bytes);
        Ok(())
    }
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("pure analysis cannot wait")
    }
}
#[test]
fn ordinary_checked_plot_plans_and_measures_real_pcm_in_production_kernel() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_audio_time_analysis_catalog(&mut startup, &mut profile)
        .unwrap();
    let source = "plot measure (\n >> pcm: PcmFrames\n observation: AudioTimeAnalysisObservation >>\n) {\n analyzer: audio/measure-time(clock-id: 7, sample-rate-hz: 8000, offset: 0, frames: 4, maximum-blocks: 1)\n pcm >> analyzer.pcm\n analyzer.observation >> observation\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "measure", &profile).unwrap();
    let mut host = StdHost::new();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "pcm"),
        (PortDirection::Output, "observation"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 65536,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 65536,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    plan.verify().unwrap();
    assert_eq!(
        plan.fragments[0].placements[0].implementation_id.as_str(),
        conduit_std_offers::AUDIO_TIME_ANALYSIS_IMPLEMENTATION
    );
    let payload: Vec<_> = [0_f32, 1., 0., -1.]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let bytes = PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        8000,
        PcmChannelLayout::Mono,
        4,
        7,
        123,
        false,
    )
    .unwrap()
    .encode_frame(&payload)
    .unwrap();
    let expected = PreparedPcmTimeAnalysis::prepare(&bytes, 7, 8000, 0, 4)
        .unwrap()
        .native_observation()
        .unwrap();
    let mut output = Output::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "pcm".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(output.0.len(), 1);
    assert_eq!(
        AudioTimeAnalysisObservation::decode(&output.0[0]).unwrap(),
        expected
    );
    let kernel = report.kernel.expect("actual production kernel report");
    assert!(!kernel.kernel_sign.is_empty());
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.active_play.plan_id, plan.plan_id);
}
