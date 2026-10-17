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
fn checked_plan() -> (StdHost, conduit_core::Plan) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_audio_spectral_analysis_catalog(&mut startup, &mut profile)
        .unwrap();
    let source = include_str!("../../../plots/audio-spectrum-analysis/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "measure", &profile).unwrap();
    let host = StdHost::new();
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
    assert!(conduit_core::verify_plan(&plan));
    assert_eq!(
        plan.fragments[0].placements[0].implementation_id.as_str(),
        conduit_std_offers::AUDIO_SPECTRAL_ANALYSIS_IMPLEMENTATION
    );
    (host, plan)
}
fn pcm(values: &[f32], clock: u64, rate: u32, discontinuity: bool) -> Vec<u8> {
    let payload: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        rate,
        PcmChannelLayout::Mono,
        values.len() as u16,
        clock,
        123,
        discontinuity,
    )
    .unwrap()
    .encode_frame(&payload)
    .unwrap()
}
#[test]
fn ordinary_checked_plot_emits_all_overlapping_spectral_rows() {
    let (mut host, plan) = checked_plan();
    let values: Vec<f32> = (0..64)
        .map(|i| (std::f64::consts::TAU * i as f64 / 8.).sin() as f32)
        .collect();
    let bytes = pcm(&values, 7, 8000, false);
    let profile = PcmSpectralProfile::new(
        PcmSpectralGeometry::new(64, 32, 16).unwrap(),
        7,
        8000,
        PcmSpectralWindow::PeriodicHann,
    )
    .unwrap();
    let mut scratch = PcmSpectralWorkspace::default();
    let expected: Vec<_> = (0..3)
        .map(|index| {
            profile
                .prepare_observation(&mut scratch, &bytes, index)
                .unwrap()
        })
        .collect();
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
    let actual: Vec<_> = output
        .0
        .iter()
        .map(|row| AudioSpectrumObservation::decode(row).unwrap())
        .collect();
    assert_eq!(actual, expected);
    let kernel = report.kernel.expect("production kernel");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.active_play.plan_id, plan.plan_id);
    assert!(!kernel.kernel_sign.is_empty());
}

#[test]
fn ordinary_kernel_refuses_invalid_whole_pcm_before_any_row() {
    let mut nonfinite = [0.; 64];
    nonfinite[63] = f32::NAN;
    for bytes in [
        pcm(&[0.; 64], 8, 8000, false),
        pcm(&[0.; 64], 7, 16000, false),
        pcm(&nonfinite, 7, 8000, false),
        pcm(&[0.; 63], 7, 8000, false),
        pcm(&[0.; 64], 7, 8000, true),
    ] {
        let (mut host, plan) = checked_plan();
        let mut output = Output::default();
        let failure = host
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
            .expect_err("invalid whole PCM");
        assert!(failure.contains("InvalidInput"), "{failure}");
        assert!(output.0.is_empty());
    }
}
