#![cfg(all(feature = "std", feature = "plot-catalog"))]
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
mod common;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn fixture() -> conduit_face::PlannedMaskPlot {
    let plot = common::checked_renderer_plot();
    let host = common::host(
        "fixture/codec-host",
        "fixture/codec-boot",
        "fixture/renderer",
        "fixture/renderer-impl",
        "fixture/renderer-artifact",
        "fixture/target",
        common::DOM_RESOURCE,
    );
    let plan = common::plan_for(&plot, host);
    let face = common::face(&plot, &plan);
    common::available_mask_show(&face).planned_mask
}

#[test]
fn hosted_mask_codec_peak_fits_named_bound_and_preflight_allocates_nothing() {
    let mask = fixture();
    let (workspace, counted) = allocation_probe::observe(|| mask.json_workspace().unwrap());
    assert_eq!((counted.allocations, counted.reallocations), (0, 0));
    let ordinary = serde_json::to_vec(&mask).unwrap();
    let output = ordinary.len().max(128).next_power_of_two();
    // The outer serde_json Vec is owned by this caller. Include its previous
    // allocation conservatively across growth; the codec receipt excludes it.
    let total = workspace.codec_bytes + output + output / 2;
    let (encoded, measured) = allocation_probe::observe(|| serde_json::to_vec(&mask).unwrap());
    assert_eq!(encoded, ordinary);
    assert!(
        measured.peak_bytes <= total,
        "actual={} bound={total}",
        measured.peak_bytes
    );
    assert_eq!(
        workspace.raw_plan_bytes,
        serde_json::to_vec(&mask.plan).unwrap().len()
    );
}

#[test]
fn oversized_raw_plan_refuses_before_raw_buffer_or_compressor_allocation() {
    let mut mask = fixture();
    mask.plan.fragments[0].placements[0].artifact_id = "x".repeat(256 * 1024 + 1).into();
    let (bound, counted) = allocation_probe::observe(|| mask.json_workspace());
    assert!(bound.is_none());
    assert_eq!((counted.allocations, counted.reallocations), (0, 0));
    let (encoded, measured) = allocation_probe::observe(|| serde_json::to_vec(&mask));
    assert!(encoded.is_err());
    // Only the caller's initial128B output Vec and finite error may exist.
    // The fixed miniz workspace alone would exceed this by two orders.
    assert!(measured.peak_bytes < 1024, "{}", measured.peak_bytes);
}

#[test]
fn pinned_compressor_receipt_cannot_silently_follow_a_dependency_upgrade() {
    assert!(include_str!("../Cargo.toml").contains("version = \"=0.8.9\""));
}
