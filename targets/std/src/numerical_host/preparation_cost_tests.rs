//! Local opt-in observation of the ordinary numerical preparation boundary.
use super::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use std::time::Instant;

#[test]
#[ignore = "local preparation-cost observation, not a calibrated timing gate"]
fn observe_numerical_preparation_cost() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = "plot project (\n >> value: Quantity <= 1800B\n result: NearQuantity <= 16384B >>\n) {\n boundary: math/near(limit=0.0001m)\n value >> boundary.value\n boundary.result >> result\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "project", &profile).unwrap();
    let mut host = StdHost::new();
    let seed = host.advertisement().clone();
    let start = Instant::now();
    let allocations = crate::allocation_probe::begin();
    host.prepare_numerical_realizations(&authored.expanded)
        .unwrap();
    let cold_allocations = allocations.finish();
    let cold_ns = start.elapsed().as_nanos();
    let original = host.advertisement().clone();
    let start = Instant::now();
    let allocations = crate::allocation_probe::begin();
    for _ in 0..32 {
        host.prepare_numerical_realizations(&authored.expanded)
            .unwrap();
        assert_eq!(host.advertisement(), &original);
    }
    let warm_allocations = allocations.finish();
    let warm_ns = start.elapsed().as_nanos();
    let configuration = &authored.expanded.gears[0].configuration;
    let expected = installed_std::near_offers(configuration).unwrap();
    let start = Instant::now();
    let allocations = crate::allocation_probe::begin();
    for _ in 0..32 {
        assert_eq!(installed_std::near_offers(configuration).unwrap(), expected);
    }
    let fragment_allocations = allocations.finish();
    let fragment_ns = start.elapsed().as_nanos();
    let start = Instant::now();
    let allocations = crate::allocation_probe::begin();
    {
        let mut owner = StdHost::from_advertisement(seed.clone()).unwrap();
        for _ in 0..33 {
            owner
                .prepare_numerical_realizations(&authored.expanded)
                .unwrap();
            assert_eq!(owner.advertisement(), &original);
        }
        // Includes final Host/cache/key/fragment destruction in the boundary.
        drop(owner);
    }
    let owner_allocations = allocations.finish();
    let owner_ns = start.elapsed().as_nanos();
    println!("OWNER_COST {{\"calls\":33,\"allocation_requests\":{owner_allocations},\"elapsed_ns_uncalibrated\":{owner_ns}}}");
    println!("PREPARATION_COST {{\"schema\":\"conduit.numeric-preparation-observation@1\",\"repetitions\":32,\"ordinary_cold_allocations\":{cold_allocations},\"ordinary_cold_ns_uncalibrated\":{cold_ns},\"ordinary_warm_allocations\":{warm_allocations},\"ordinary_warm_ns_uncalibrated\":{warm_ns},\"fragment_allocations\":{fragment_allocations},\"fragment_ns_uncalibrated\":{fragment_ns}}}");
}
