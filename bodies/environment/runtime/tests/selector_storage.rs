//! Type-domain storage parity for actual prepared pulse-result selectors.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{self as c, StructuredSelector, UnmatchedVariantDisposition};
use conduit_environment_runtime::PreparedCaptureSelector;
use conduit_pulse_capture::{
    PreparedPulseCodec, PulseReadCapture, PulseReadContract, PulseReadDisposition,
};

#[test]
fn full_type_domains_retain_exact_selection_with_smaller_output_backing() {
    let contract = PulseReadContract::prepare().unwrap();
    let mut codec = PreparedPulseCodec::prepare(&contract).unwrap();
    let mut inputs = Vec::new();
    for disposition in [
        PulseReadDisposition::Complete,
        PulseReadDisposition::TimedOut,
    ] {
        for count in 0..=8 {
            if disposition == PulseReadDisposition::Complete && count == 0 {
                continue;
            }
            for byte in [0, 255] {
                let capture =
                    PulseReadCapture::from_complete_octets(disposition, &vec![byte; count])
                        .unwrap();
                inputs.push(codec.result(capture).unwrap().to_vec());
            }
        }
    }
    inputs.push(
        codec
            .result(
                PulseReadCapture::from_complete_octets(PulseReadDisposition::ProviderFailed, &[])
                    .unwrap(),
            )
            .unwrap()
            .to_vec(),
    );
    inputs.push(codec.geometry_refused().unwrap().to_vec());
    let wrong = c::StructuredInfoValue::leaf(
        c::StructuredInfoType::leaf(c::kind_id(c::EMPTY_INFO_ID)).unwrap(),
        vec![],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let over_input = [0; 1025];
    let mut retained_saved = 0;
    let mut extents = Vec::new();
    for tag in [
        "completed",
        "timed-out",
        "provider-failed",
        "geometry-refused",
    ] {
        let selector = StructuredSelector::variant(
            contract.result_type().clone(),
            tag,
            UnmatchedVariantDisposition::Drop,
        )
        .unwrap();
        let extent = conduit_plot::maximum_prepared_canonical_value_bytes(selector.output_type())
            .unwrap() as usize;
        extents.push(extent);
        let (mut original, before) = allocation_probe::observe(|| {
            c::PreparedCanonicalStructuredSelector::prepare(&selector, 1024, 1024).unwrap()
        });
        let (mut prepared, after) = allocation_probe::observe(|| {
            PreparedCaptureSelector::prepare(selector.clone(), 1024, 1024).unwrap()
        });
        let input_schema = Box::leak(
            selector
                .input_type()
                .canonical_bytes()
                .unwrap()
                .into_boxed_slice(),
        );
        let output_schema = Box::leak(
            selector
                .output_type()
                .canonical_bytes()
                .unwrap()
                .into_boxed_slice(),
        );
        let (mut linked, linked_memory) = allocation_probe::observe(|| {
            PreparedCaptureSelector::with_static_schema(
                selector.clone(),
                input_schema,
                output_schema,
                1024,
                1024,
            )
            .unwrap()
        });
        assert_eq!(
            after.retained_bytes() - linked_memory.retained_bytes(),
            input_schema.len() + output_schema.len()
        );
        assert_eq!(
            before.retained_bytes() - after.retained_bytes(),
            1024 - extent
        );
        retained_saved += before.retained_bytes() - after.retained_bytes();
        let mut maximum_selected = 0;
        let ((), memory) = allocation_probe::observe(|| {
            for input in &inputs {
                let selected = prepared.select(input).unwrap();
                maximum_selected = maximum_selected.max(selected.map_or(0, <[u8]>::len));
                assert_eq!(selected, original.select(input).unwrap());
                assert_eq!(selected, linked.select(input).unwrap());
            }
            assert_eq!(prepared.select(&wrong), original.select(&wrong));
            assert_eq!(prepared.select(&wrong), linked.select(&wrong));
            assert!(prepared.select(&wrong).is_err());
            assert_eq!(prepared.select(&over_input), original.select(&over_input));
            assert_eq!(prepared.select(&over_input), linked.select(&over_input));
            assert!(prepared.select(&over_input).is_err());
            let truncated = &inputs[0][..inputs[0].len() - 1];
            assert_eq!(prepared.select(truncated), original.select(truncated));
            assert_eq!(prepared.select(truncated), linked.select(truncated));
            assert!(prepared.select(truncated).is_err());
        });
        // A legal value reaches the entire reserved Type domain, so this is a
        // checked worst-case bound rather than a typical sensor transcript size.
        assert_eq!(maximum_selected, extent);
        assert_eq!(memory.reallocations, 0);
        assert_eq!(memory.allocations, 0);
    }
    eprintln!(
        "pulse_selector_type_extents={extents:?} retained_output_bytes_saved={retained_saved}"
    );
    assert_eq!(extents, [224, 224, 21, 21]);
    assert_eq!(retained_saved, 3606);
}
