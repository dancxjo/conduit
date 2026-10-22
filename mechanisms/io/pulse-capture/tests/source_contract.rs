use conduit_core::{validate_canonical_structured_value as validate, ConfigurationValue};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_pulse_capture::*;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn source() -> String {
    format!(
        "{}\n{}",
        include_str!("../../../../plots/device-protocols/pulse-types.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-capture.conduit")
    )
}
fn request() -> Vec<u8> {
    let syntax = parse_syntax_document(&source());
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "dht11-capture-request",
        &ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    let ConfigurationValue::Text(program) = &expanded.gears[0].configuration[0].value else {
        panic!("Source expression")
    };
    PortableExpressionProgram::from_canonical_hex(program)
        .unwrap()
        .evaluate(&[])
        .unwrap()
}
fn limits() -> PulseReadLimits {
    PulseReadLimits {
        maximum_hold_low_us: 50_000,
        maximum_skipped_pulses: 4,
        minimum_sample_offset_us: 10,
        maximum_sample_offset_us: 200,
        maximum_octets: 8,
        maximum_capture_timeout_us: 50_000,
    }
}

#[test]
fn actual_source_request_crosses_exact_codec_without_allocating() {
    let contract = PulseReadContract::prepare().unwrap();
    let codec = PreparedPulseCodec::prepare(&contract).unwrap();
    let input = request();
    let (checked, measured) =
        allocation_probe::observe(|| codec.request(&input, limits()).unwrap());
    assert_eq!(measured.allocations, 0);
    assert_eq!(measured.reallocations, 0);
    assert_eq!(
        checked.request(),
        PulseReadRequest {
            hold_low_us: 20_000,
            skip_high_pulses: 1,
            sample_after_rise_us: 40,
            octets: 5,
            capture_timeout_us: 10_000
        }
    );
    assert!(codec.request(&input[..input.len() - 1], limits()).is_err());
    assert!(matches!(
        codec.request(
            &input,
            PulseReadLimits {
                maximum_octets: 4,
                ..limits()
            }
        ),
        Err(PulseCodecRefusal::Geometry(PulseReadRefusal::OctetExtent))
    ));
}

#[test]
fn typed_timeout_and_provider_loss_preserve_extent_without_allocating() {
    let contract = PulseReadContract::prepare().unwrap();
    let mut codec = PreparedPulseCodec::prepare(&contract).unwrap();
    let timeout =
        PulseReadCapture::from_complete_octets(PulseReadDisposition::TimedOut, &[0x80, 1]).unwrap();
    let (_, measured) = allocation_probe::observe(|| {
        for _ in 0..64 {
            assert!(validate(
                codec
                    .result(
                        PulseReadCapture::from_complete_octets(
                            PulseReadDisposition::Complete,
                            &[0x81]
                        )
                        .unwrap()
                    )
                    .unwrap()
            )
            .unwrap()
            .variant_payload("completed")
            .unwrap()
            .is_some());
            let bytes = codec.result(timeout).unwrap();
            let capture = validate(bytes)
                .unwrap()
                .variant_payload("timed-out")
                .unwrap()
                .unwrap();
            let length = capture.record_field("length").unwrap().unwrap();
            assert_eq!(&length.value_node()[5..], &[2]);
            assert!(validate(
                codec
                    .result(
                        PulseReadCapture::from_complete_octets(
                            PulseReadDisposition::ProviderFailed,
                            &[]
                        )
                        .unwrap()
                    )
                    .unwrap()
            )
            .unwrap()
            .variant_payload("provider-failed")
            .unwrap()
            .is_some());
            assert!(validate(codec.geometry_refused().unwrap())
                .unwrap()
                .variant_payload("geometry-refused")
                .unwrap()
                .is_some());
        }
    });
    assert_eq!(measured.allocations, 0);
    assert_eq!(measured.reallocations, 0);
    let mut dirty = timeout;
    dirty.wire[7] = 1;
    assert_eq!(codec.result(dirty), Err(PulseCodecRefusal::InvalidCapture));
}

#[test]
#[cfg(feature = "plot-catalog")]
fn source_request_composes_over_the_class_neutral_fore() {
    let contract = PulseReadContract::prepare().unwrap();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    contract.install(&mut startup, &mut profiles).unwrap();
    let source=format!("{}\nplot read (\n >> begin: Empty...|\n result: PulseReadResult...| >>\n) {{\n begin >> dht11-capture-request() >> machine/pulse/read() >> result\n}}\n",source());
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "read", &profiles)
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 2);
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == PULSE_READ_KIND));
}

#[test]
#[cfg(feature = "plot-catalog")]
fn the_source_read_graph_preserves_electrical_outcomes_on_distinct_ports() {
    let contract = PulseReadContract::prepare().unwrap();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    contract.install(&mut startup, &mut profiles).unwrap();
    let source = format!(
        "{}\n{}\n{}",
        source(),
        include_str!("../../../../plots/device-protocols/dht11.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-read.conduit")
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    for plot in &checked.plots {
        for cord in &plot.cords {
            for stage in &cord.stages {
                if let conduit_plot::CheckedCordStage::StructuredSelector { selector, .. } = stage {
                    profiles
                        .insert(conduit_plot::structured_selector_definition(
                            selector,
                            conduit_core::PortTemporal::Flow { closes: true },
                        ))
                        .unwrap();
                }
            }
        }
    }
    let authoring = expand_canonical_plot_for_authoring(&checked, "dht11-read", &profiles).unwrap();
    assert_eq!(authoring.expanded.gears.len(), 8);
    let mut outputs = authoring
        .output_bindings
        .iter()
        .map(|binding| binding.front_port_id.as_str())
        .collect::<Vec<_>>();
    outputs.sort();
    assert_eq!(
        outputs,
        [
            "decoded",
            "geometry_refused",
            "provider_failed",
            "timed_out"
        ]
    );
    assert_eq!(
        authoring
            .expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == PULSE_READ_KIND)
            .count(),
        1
    );
}
