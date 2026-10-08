#![cfg(feature = "fargan-native-profile-preparation")]
extern crate alloc;
#[path = "../../../semantics/ai/src/numeric_allocation_probe.rs"]
mod allocation_probe;
use conduitos::fargan_native_profile_preparation as preparation;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use preparation::*;
use std::{path::PathBuf, sync::Arc};
fn inputs() -> Inputs {
    let root = PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_PREPARATION_SOURCE_ROOT")
            .expect("set exact original declaration Source root"),
    );
    let project = PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_PREPARATION_ARTIFACT_ROOT")
            .expect("set exact archived FARGAN artifact root"),
    );
    Inputs {
        plan: std::fs::read(
            project.join("outputs/committed-common-fargan-greeting/sealed-epoch-plan.json"),
        )
        .unwrap()
        .into(),
        source: std::fs::read_to_string(
            project.join("outputs/committed-common-fargan-greeting/checked-epoch-source.conduit"),
        )
        .unwrap()
        .into(),
        declarations: DECLARATION_PATHS
            .map(|p| std::fs::read_to_string(root.join(p)).unwrap().into()),
        resources: ["f32.bin", "compact.bin", "manifest.json"].map(|p| {
            std::fs::read(project.join("work/fargan/resources").join(p))
                .unwrap()
                .into()
        }),
    }
}
fn limits(_i: &Inputs) -> Limits {
    Limits {
        preparation_requested_bytes: PREPARATION_REQUESTED_BYTES,
        preparation_peak_bytes: PREPARATION_PEAK_BYTES,
        existing_input_bytes: EXISTING_INPUT_OWNER_BYTES,
    }
}
#[test]
fn one_under_refuses_before_allocating() {
    for which in 0..3 {
        let i = inputs();
        let mut l = limits(&i);
        match which {
            0 => l.preparation_requested_bytes -= 1,
            1 => l.preparation_peak_bytes -= 1,
            _ => l.existing_input_bytes -= 1,
        };
        let (r, o) = allocation_probe::observe(|| PreparedNativeProfiles::prepare(i, l));
        assert!(matches!(r, Err(Refusal::Capacity)));
        assert_eq!((o.allocations, o.reallocations), (0, 0));
    }
}
#[test]
fn foreign_full_inputs_refuse_without_allocation() {
    for which in 0..4 {
        let mut i = inputs();
        match which {
            0 => {
                let mut v = i.plan.to_vec();
                v[0] ^= 1;
                i.plan = v.into()
            }
            1 => i.source = "foreign".into(),
            2 => i.declarations[10] = "foreign".into(),
            _ => i.resources[0] = Arc::from(&b"foreign"[..]),
        };
        let l = limits(&i);
        let (r, o) = allocation_probe::observe(|| PreparedNativeProfiles::prepare(i, l));
        assert!(matches!(
            r,
            Err(Refusal::ForeignPlan
                | Refusal::ForeignSource
                | Refusal::ForeignDeclaration(_)
                | Refusal::ForeignResource(_))
        ));
        assert_eq!((o.allocations, o.reallocations), (0, 0));
    }
}
#[test]
fn complete_actual_preparation_preserves_originals_within_admitted_bounds() {
    let (i, input_observed) = allocation_probe::observe(inputs);
    assert!(input_observed.live_bytes <= EXISTING_INPUT_OWNER_BYTES);
    println!(
        "original input owner live={} construction requested={} peak={} (construction upstream)",
        input_observed.live_bytes, input_observed.requested_bytes, input_observed.peak_bytes
    );
    let p = Arc::clone(&i.plan);
    let m = Arc::clone(&i.resources[0]);
    let source = Arc::clone(&i.source);
    let l = limits(&i);
    let (r, o) = allocation_probe::observe(|| PreparedNativeProfiles::prepare(i, l));
    let prepared = r.unwrap();
    let receipt = prepared.receipt();
    assert_eq!(
        receipt.original_input_owner_bytes_bound(),
        EXISTING_INPUT_OWNER_BYTES
    );
    assert!(o.requested_bytes <= receipt.preparation_requested_bytes_bound());
    assert!(o.peak_bytes <= receipt.preparation_peak_bytes_bound());
    assert!(Arc::ptr_eq(&p, &prepared.inputs().plan));
    assert!(Arc::ptr_eq(&m, &prepared.inputs().resources[0]));
    assert!(Arc::ptr_eq(&source, &prepared.inputs().source));
    assert_eq!(prepared.profiles().len(), 18);
    assert_eq!(
        prepared
            .plan()
            .fragments
            .iter()
            .map(|f| f.placements.len())
            .sum::<usize>(),
        958
    );
    println!(
        "requested={} peak={} live={} retained_inputs={} factory={:?}",
        o.requested_bytes,
        o.peak_bytes,
        o.live_bytes,
        receipt.original_input_payload_bytes(),
        prepared.factory().storage().unwrap()
    );
}
