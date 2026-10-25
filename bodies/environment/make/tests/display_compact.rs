//! Borrowed Make metadata advertises exactly the initialized ordinary owners.
use conduit_environment_make::{DisplayOutputSource, DisplaySourceMaterial};
#[test]
fn compact_owners_preserve_full_offers_and_current_scope() {
    let source = DisplayOutputSource::compile().unwrap();
    let material = DisplaySourceMaterial::prepare(&source).unwrap();
    let host: conduit_core::HostId = "fixture/display".into();
    let boot: conduit_core::BootId = "fixture/boot".into();
    let mut owned = material
        .prepare_software(host.clone(), boot.clone())
        .unwrap();
    let mut compact = material
        .prepare_compact(host.clone(), boot.clone())
        .unwrap();
    let generation = conduit_core::OfferGeneration(7);
    let expected: Vec<_> = owned
        .current_offers(&host, &boot, generation)
        .unwrap()
        .iter()
        .cloned()
        .collect();
    let actual: Vec<conduit_core::CapabilityOffer> = compact
        .current_offers(&host, &boot, generation)
        .unwrap()
        .iter()
        .map(|json| serde_json::from_slice(json).unwrap())
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 3);
    assert!(compact
        .current_offers(&host, &boot, conduit_core::OfferGeneration(6))
        .is_err());
    assert!(compact
        .current_offers(&host, &"stale/boot".into(), generation)
        .is_err());
    assert!(compact
        .current_offers(&"foreign/host".into(), &boot, generation)
        .is_err());
    assert!(compact
        .current_offers(&host, &boot, conduit_core::OfferGeneration(0))
        .is_err());
    assert_eq!(
        compact
            .current_offers(&host, &boot, generation)
            .unwrap()
            .iter()
            .count(),
        3
    );
}
