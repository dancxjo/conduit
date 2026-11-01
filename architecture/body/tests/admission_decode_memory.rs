#![cfg(feature = "authenticated-admission")]
use conduit_body::admission_advertisement_wire::CompactAdvertisement;
use conduit_core as c;
#[path = "common/admission_allocation_probe.rs"]
mod allocation;
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;

#[test]
fn complete_decode_retires_compressed_storage_before_metadata_construction() {
    let mut seed = 7u32;
    let text: String = (0..16_000)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            char::from(b'!' + (seed % 90) as u8)
        })
        .collect();
    let ad = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: text.into(),
        boot_id: "boot/memory".into(),
        offer_generation: c::OfferGeneration(3),
        profile: "profile/memory".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let wire = serde_json::to_vec(&CompactAdvertisement(ad.clone())).unwrap();
    let (decoded, observed) =
        allocation::observe(|| serde_json::from_slice::<CompactAdvertisement>(&wire));
    assert_eq!(decoded.unwrap().0, ad);
    // CPU allocator envelope for this exact deterministic 27 KiB carrier.
    // Excludes preexisting wire/expected descriptor and decoder stack storage;
    // it is a regression guard, not an MCU workspace admission grant.
    assert!(observed.peak_bytes <= 60 * 1024, "{observed:?}");
    assert!(observed.retained_bytes() >= 16_000, "{observed:?}");
    // The expanded document is the decoder's largest exact allocation. A real
    // allocator refusal must return a decode error rather than abort the Host.
    let expanded_bytes = serde_json::to_vec(&ad).unwrap().len();
    let guard = allocation::refuse_next_allocation_size(expanded_bytes);
    assert!(serde_json::from_slice::<CompactAdvertisement>(&wire).is_err());
    drop(guard);
}

/// Optional accounting of an actual, privately retained complete Host offer.
/// Supply only a local path; this proof performs no transport or Host effects.
#[test]
#[ignore = "requires CONDUIT_ADMISSION_MEASUREMENT_ADVERTISEMENT pointing to a private complete offer"]
fn measure_actual_complete_advertisement_decode() {
    let path = std::env::var_os("CONDUIT_ADMISSION_MEASUREMENT_ADVERTISEMENT").unwrap();
    use std::io::Read;
    let mut input = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .take(64 * 1024 + 1)
        .read_to_end(&mut input)
        .unwrap();
    assert!(input.len() <= 64 * 1024);
    let expected: c::HostAdvertisement = serde_json::from_slice(&input).unwrap();
    let wire = serde_json::to_vec(&CompactAdvertisement(expected.clone())).unwrap();
    let (decoded, measured) =
        allocation::observe(|| serde_json::from_slice::<CompactAdvertisement>(&wire));
    assert_eq!(decoded.unwrap().0, expected);
    eprintln!("complete-offer expanded={} carrier={} decode_peak={} retained={} allocations={} reallocations={}; CPU allocator only, excludes preexisting input/expected offer and stack", input.len(), wire.len(), measured.peak_bytes, measured.retained_bytes(), measured.allocations, measured.reallocations);
}
