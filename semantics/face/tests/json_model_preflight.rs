//! Admission must precede owning serde, including adversarial escaped strings.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_face::{json_model_preflight::generic_face_json_shape, PlannedMaskPlot};

#[test]
fn generic_face_fences_are_allocation_free_for_escaped_text_and_refusals() {
    for (bytes, accepted) in [
        (br#"{"composition":[],"constraints":[],"arguments":[],"text":"quoted \"value\"\n\u2026"}"#.as_slice(), true),
        (br#"{"composition":[{}]}"#, false),
        (br#"{"properties":[{"contract":{"constraints":[{}]}}]}"#, false),
        (br#"{"actions":[{"arguments":[{}]}]}"#, false),
        (br#"{"\u0063omposition":[]}"#, false),
        (br#"{"text":"composition:[{}]","composition": [ \t ]}"#, false),
        (br#"{"text":"composition:[{}]","composition": [ ]}"#, true),
        (br#"{"composition":[] true}"#, false),
    ] {
        let (result, seen) = allocation_probe::observe(|| generic_face_json_shape(bytes));
        assert_eq!(result.is_ok(), accepted);
        assert_eq!((seen.allocation_attempts, seen.allocations, seen.reallocations), (0, 0, 0));
    }
}

#[test]
fn generic_plan_fences_allocate_nothing_before_model_admission() {
    for (bytes, accepted) in [
        (br#"{"fragments":[{"placements":[{"configuration":[],"semantic_contract":{"configuration":[],"laws":[]},"resources":[{"protected":null,"compute":null}]}]}],"id":"quoted \"host\"\n"}"#.as_slice(), true),
        (br#"{"activations":[{}]}"#, false),
        (br#"{"resources":[{"content":{}}]}"#, false),
        (br#"{"\u0063onfiguration":[]}"#, false),
        (br#"{"configuration":[],"unknown":"escaped\nvalue"}"#, true),
    ] {
        let (result, seen) = allocation_probe::observe(|| PlannedMaskPlot::generic_json_plan_preparation_bytes(bytes));
        assert_eq!(result.is_ok(), accepted);
        assert_eq!((seen.allocation_attempts, seen.allocations, seen.reallocations), (0, 0, 0));
    }
}

#[test]
fn pinned_plan_codec_workspace_covers_maximum_incompressible_input() {
    let mut input = vec![0; 256 * 1024];
    let mut state = 0x9e37_79b9u32;
    for byte in &mut input {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *byte = state as u8;
    }
    let (compressed, seen) =
        allocation_probe::observe(|| miniz_oxide::deflate::compress_to_vec_zlib(&input, 6));
    assert!(
        compressed.len() > 64 * 1024,
        "exercise pre-refusal compressor growth"
    );
    assert!(compressed.capacity() <= 512 * 1024);
    assert!(seen.peak_bytes <= PlannedMaskPlot::json_wire_preparation_bytes());
    assert_eq!(seen.refused_allocations, 0);
}
