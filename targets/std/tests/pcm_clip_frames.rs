use conduit_audio::{
    encode_pcm_clip, AudioTimeAnalysisObservation, PcmChannelLayout, PcmFrameHeader,
    PcmSampleRepresentation,
};
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
struct Output(Vec<conduit_std_host::ExternalForeBytes>);
impl ExternalForeOutputAdapter for Output {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(output.bytes);
        Ok(())
    }
}
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("pure clip expansion cannot wait on an external clock")
    }
}
fn checked(source: &str, entry: &str, outputs: &[&str]) -> (StdHost, conduit_core::Plan) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_audio_clip_frames_catalog(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_audio_time_analysis_catalog(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_audio_spectral_analysis_catalog(&mut startup, &mut profile)
        .unwrap();
    let document = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&document, entry, &profile).unwrap();
    let host = StdHost::new();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = core::iter::once((PortDirection::Input, "clip"))
        .chain(outputs.iter().map(|name| (PortDirection::Output, *name)))
        .map(|(direction, name)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: if direction == PortDirection::Input {
                        conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
                    } else {
                        65536
                    },
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
    (host, plan)
}
const EXPAND: &str = include_str!("../../../plots/audio-clip-frames/main.conduit");
const ANALYZE: &str = r#"plot measure (
    >> clip: PcmClip
    result: AudioTimeAnalysisObservation...| >>
) {
    frames: audio/clip-to-frames
    analysis: audio/measure-time(clock-id = 7, sample-rate-hz = 16000, frames = 4, maximum-blocks = 64)
    clip >> frames.clip
    frames.pcm >> analysis.pcm
    analysis.observation >> result
}."#;

fn frame(start: u64, samples: &[i16]) -> Vec<u8> {
    PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        16000,
        PcmChannelLayout::Mono,
        samples.len() as u16,
        7,
        start,
        false,
    )
    .unwrap()
    .encode_frame(
        &samples
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
#[test]
fn ordinary_checked_plan_drains_all_original_frames_including_short_tail_and_closes() {
    let blocks = [frame(31, &[1, 2, 3, 4]), frame(35, &[5])];
    let bytes = encode_pcm_clip(&[&blocks[0], &blocks[1]]).unwrap();
    let (mut host, plan) = checked(EXPAND, "unpack", &["pcm"]);
    let mut output = Output::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "clip".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert!(output
        .0
        .iter()
        .map(|bytes| bytes.as_slice())
        .eq(blocks.iter().map(Vec::as_slice)));
    let kernel = report.kernel.expect("actual production kernel report");
    assert_eq!(kernel.active_play.plan_id, plan.plan_id);
    assert!(!kernel.kernel_sign.is_empty());
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
#[test]
fn captured_clip_shape_runs_through_frame_flow_to_typed_time_observations() {
    let first = frame(31, &[16384; 4]);
    let second = frame(35, &[-8192; 4]);
    let bytes = encode_pcm_clip(&[&first, &second]).unwrap();
    let (mut host, plan) = checked(ANALYZE, "measure", &["result"]);
    let mut output = Output::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "clip".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(output.0.len(), 2);
    for (i, bytes) in output.0.iter().enumerate() {
        let value = AudioTimeAnalysisObservation::decode(bytes).unwrap();
        assert_eq!(value.rms().get().value(), if i == 0 { 0.5 } else { 0.25 });
        assert_eq!(*value.clock_id(), 7);
        assert_eq!(*value.start_frame(), 31 + i as u64 * 4);
    }
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
#[test]
fn invalid_tail_never_releases_an_apparently_valid_prefix() {
    let first = frame(31, &[1, 2]);
    let second = frame(33, &[3]);
    let mut bytes = encode_pcm_clip(&[&first, &second]).unwrap();
    bytes.pop();
    let (mut host, plan) = checked(EXPAND, "unpack", &["pcm"]);
    let mut output = Output::default();
    assert!(host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "clip".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer
        )
        .is_err());
    assert!(output.0.is_empty());
}

#[derive(Default)]
struct Analyses {
    waveform: Vec<conduit_std_host::ExternalForeBytes>,
    spectrum: Vec<conduit_std_host::ExternalForeBytes>,
}
impl ExternalForeOutputAdapter for Analyses {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        match output.front_port_id.as_str() {
            "waveform" => self.waveform.push(output.bytes),
            "spectrum" => self.spectrum.push(output.bytes),
            _ => return Err("unexpected analysis Fore".into()),
        }
        Ok(())
    }
}
#[test]
fn one_checked_play_fans_out_the_same_frames_to_waveform_and_overlapping_spectrum() {
    let source = r#"plot measure (
    >> clip: PcmClip
    waveform: AudioTimeAnalysisObservation...| >>
    spectrum: AudioSpectrumObservation...| >>
) {
    frames: audio/clip-to-frames
    time: audio/measure-time(clock-id = 7, sample-rate-hz = 16000, frames = 64, maximum-blocks = 64)
    fft: audio/measure-spectrum(clock-id = 7, sample-rate = 16000Hz, source-frames = 64, window-frames = 32, stride-frames = 16, window = "rectangular", maximum-blocks = 64)
    clip >> frames.clip
    frames.pcm >> time.pcm
    frames.pcm >> fft.pcm
    time.observation >> waveform
    fft.observation >> spectrum
}."#;
    let first = frame(31, &[16384; 64]);
    let second = frame(95, &[-8192; 64]);
    let bytes = encode_pcm_clip(&[&first, &second]).unwrap();
    let (mut host, plan) = checked(source, "measure", &["waveform", "spectrum"]);
    let mut output = Analyses::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "clip".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!((output.waveform.len(), output.spectrum.len()), (2, 6));
    for (index, bytes) in output.waveform.iter().enumerate() {
        let waveform = AudioTimeAnalysisObservation::decode(bytes).unwrap();
        let expected = if index == 0 { 0.5 } else { 0.25 };
        assert_eq!(waveform.rms().get().value(), expected);
        for row in 0..3 {
            let spectrum =
                conduit_audio::AudioSpectrumObservation::decode(&output.spectrum[index * 3 + row])
                    .unwrap();
            assert_eq!(
                spectrum.source_digest().get(),
                waveform.source_digest().get()
            );
            assert_eq!(spectrum.clock_id(), waveform.clock_id());
            assert_eq!(spectrum.source_start_frame(), waveform.start_frame());
            assert_eq!(
                *spectrum.window_start_frame(),
                *waveform.start_frame() + row as u64 * 16
            );
            assert_eq!(spectrum.peak_amplitudes().get()[0].get().value(), expected);
        }
    }
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.active_play.plan_id, plan.plan_id);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

/// Historical PCM replay only: clock 7 names this finite replay, not the
/// microphone's acquisition clock or a current Body observation.
#[test]
#[ignore = "requires an explicitly supplied 2048-sample s16le mono historical excerpt"]
fn retained_microphone_excerpt_runs_one_checked_waveform_and_spectrum_play() {
    let path = std::env::var("CONDUIT_RETAINED_PCM_EXCERPT").unwrap();
    let file = std::fs::File::open(path).unwrap();
    use std::io::Read;
    let mut pcm = Vec::new();
    file.take(4097).read_to_end(&mut pcm).unwrap();
    assert_eq!(pcm.len(), 4096);
    let samples: Vec<i16> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|s| i16::from_le_bytes(*s))
        .collect();
    let source = r#"plot measure (
    >> clip: PcmClip
    waveform: AudioTimeAnalysisObservation...| >>
    spectrum: AudioSpectrumObservation...| >>
) {
    frames: audio/clip-to-frames
    time: audio/measure-time(clock-id = 7, sample-rate-hz = 16000, frames = 1024, maximum-blocks = 64)
    fft: audio/measure-spectrum(clock-id = 7, sample-rate = 16000Hz, source-frames = 1024, window-frames = 256, stride-frames = 128, window = "rectangular", maximum-blocks = 64)
    clip >> frames.clip
    frames.pcm >> time.pcm
    frames.pcm >> fft.pcm
    time.observation >> waveform
    fft.observation >> spectrum
}."#;
    let first = frame(104448, &samples[..1024]);
    let second = frame(105472, &samples[1024..]);
    let bytes = encode_pcm_clip(&[&first, &second]).unwrap();
    let (mut host, plan) = checked(source, "measure", &["waveform", "spectrum"]);
    let mut output = Analyses::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "clip".into(),
                track: ConnectionTrack::Payload,
                bytes,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!((output.waveform.len(), output.spectrum.len()), (2, 14));
    for (index, bytes) in output.waveform.iter().enumerate() {
        let time = AudioTimeAnalysisObservation::decode(bytes).unwrap();
        let expected_rms = (samples[index * 1024..(index + 1) * 1024]
            .iter()
            .map(|s| (f64::from(*s) / 32768.0).powi(2))
            .sum::<f64>()
            / 1024.0)
            .sqrt();
        assert!((time.rms().get().value() - expected_rms).abs() < 1e-12);
        assert_eq!(*time.start_frame(), 104448 + index as u64 * 1024);
        for row in 0..7 {
            let spectrum =
                conduit_audio::AudioSpectrumObservation::decode(&output.spectrum[index * 7 + row])
                    .unwrap();
            assert_eq!(spectrum.source_digest().get(), time.source_digest().get());
            assert_eq!(spectrum.clock_id(), time.clock_id());
            assert_eq!(spectrum.source_start_frame(), time.start_frame());
            assert_eq!(
                *spectrum.window_start_frame(),
                *time.start_frame() + row as u64 * 128
            );
        }
        println!(
            "retained replay block {index}: start={} rms={expected_rms}",
            time.start_frame()
        );
    }
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.active_play.plan_id, plan.plan_id);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    println!(
        "retained replay: plan={} waveform=2 spectrum=14 unchanged_value_capacity=true",
        plan.plan_id.as_str()
    );
}
