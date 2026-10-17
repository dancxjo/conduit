use conduit_audio::*;
use conduit_core::{
    ConfigurationValue, StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
pub(crate) struct Pipeline {
    pub(crate) programs: Vec<PortableExpressionProgram>,
    prepared: Vec<PreparedPortableExpressionEvaluator>,
}
impl Pipeline {
    pub(crate) fn new(entry: &str, input: StructuredInfoType) -> Self {
        let source = [
            include_str!("../../types.conduit"),
            include_str!("../../trajectory_types.conduit"),
            include_str!("../../rate_types.conduit"),
            include_str!("../../decibel_types.conduit"),
            include_str!("../../exact_control_types.conduit"),
            include_str!("../../time_analysis_types.conduit"),
            include_str!("../../dsp/periodic-types.conduit"),
            PERIODIC_FLOW_SOURCE,
            PERIODIC_OSCILLATOR_SOURCE,
            PERIODIC_VALIDATION_SOURCE,
        ]
        .join("\n");
        let syntax = parse_syntax_document(&source);
        assert_eq!(syntax.round_trip(), source);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
        let mut unordered: Vec<_> = expanded
            .expanded
            .gears
            .iter()
            .map(|gear| {
                let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
                    panic!("program")
                };
                (
                    gear.gear_id.clone(),
                    PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                )
            })
            .collect();
        let mut ty = input;
        let mut programs = Vec::new();
        while !unordered.is_empty() {
            let index = unordered
                .iter()
                .position(|(id, p)| {
                    p.input_type == ty
                        && !expanded.expanded.connections.iter().any(|edge| {
                            edge.sink_gear_id == *id
                                && unordered
                                    .iter()
                                    .any(|(upstream, _)| *upstream == edge.source_gear_id)
                        })
                })
                .expect("one linear typed pipeline");
            let (_, program) = unordered.remove(index);
            ty = program.output_type.clone();
            programs.push(program);
        }
        let prepared = programs
            .iter()
            .map(|p| PreparedPortableExpressionEvaluator::new(p).unwrap())
            .collect();
        Self { programs, prepared }
    }
    pub(crate) fn run(&mut self, mut bytes: Vec<u8>) -> Vec<u8> {
        for (program, prepared) in self.programs.iter().zip(&mut self.prepared) {
            let reference = program.evaluate(&bytes).unwrap();
            let capacity = prepared.output_capacity();
            let (actual, storage) =
                allocation_probe::observe(|| prepared.evaluate(&bytes).unwrap());
            assert_eq!(actual, reference);
            assert_eq!((storage.allocations, storage.reallocations), (0, 0));
            assert_eq!(prepared.output_capacity(), capacity);
            bytes = reference;
        }
        bytes
    }
}
fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "periodic fixture".into(),
        None,
    )
    .unwrap()
}
pub(crate) fn request(
    n: u64,
    d: u64,
    phase: i64,
    phase_den: u64,
    amp_n: u64,
    amp_d: u64,
    frames: u32,
) -> AudioPeriodicRequest {
    AudioPeriodicRequest::new(
        AudioRelativeAmplitude::new(amp_d, amp_n).unwrap(),
        AudioSampleRateBasis::new(anchor(), AudioFrameQuantization::Floor, 8000).unwrap(),
        frames,
        AudioFrequencyControl::new(anchor(), provenance(), AudioFrequencyHz::new(d, n).unwrap())
            .unwrap(),
        AudioPhaseReference::new(
            anchor(),
            AudioExactTimeOffset::new(1, 0).unwrap(),
            phase_den,
            phase,
            provenance(),
        )
        .unwrap(),
        AudioPeriodicReference::UnscaledCubicFlow,
    )
    .unwrap()
}
pub(crate) fn started(bytes: &[u8]) -> AudioPeriodicCursorCandidate {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("start outcome")
    };
    assert_eq!(tag, "started");
    AudioPeriodicCursorCandidate::decode(&payload.canonical_bytes().unwrap()).unwrap()
}
