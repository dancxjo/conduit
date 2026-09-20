use std::{fs, path::Path};

/// Production constructors already migrated under #3718. Once a path enters
/// this list it cannot regain a raw `CapabilityOffer` literal; malformed and
/// adversarial fixtures remain outside this production-only ratchet.
const MIGRATED_PRODUCTION_PATHS: &[(&str, usize, &str)] = &[
    (
        "semantics/catalog/src/final_normalized_pattern.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/pattern_comparison.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/functional_front.rs",
        0,
        "fully migrated",
    ),
    (
        "semantics/catalog/src/sequence_normalization.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/timed_pattern.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/template_storage.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/net/src/ordered_record_queue_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/net/src/record_delivery_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/net/src/record_temporal_catalog.rs",
        0,
        "semantic contract owner",
    ),
    ("semantics/signal/src/control.rs", 0, "fully migrated"),
    ("semantics/signal/src/lib.rs", 0, "fully migrated"),
    ("semantics/tongues/src/contract.rs", 0, "fully migrated"),
    (
        "targets/browser/runtime/src/installed_browser/audio_io.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/final_normalized_pattern.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/pattern_comparison.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/timing.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/template_storage.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/record_delivery.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/record_queue.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/record_temporal.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/copy_file.rs",
        1,
        "the structured copy-result presenter awaits its owning semantic contract",
    ),
    (
        "targets/std/offers/src/final_normalized_pattern.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/pattern_comparison.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/music.rs",
        2,
        "two KindDefinition families do not yet own semantic capacity",
    ),
    (
        "targets/std/offers/src/record_delivery.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/record_queue.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/record_temporal.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/signal.rs", 0, "fully migrated"),
    (
        "targets/std/offers/src/sequence_normalization.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/speech_synthesis.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/timed_pattern.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/template_storage.rs",
        0,
        "fully migrated",
    ),
];

#[test]
fn migrated_production_offers_cannot_restate_capability_truth() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is beneath repository tools");

    let mut violations = Vec::new();
    for (relative, expected_raw_literals, reason) in MIGRATED_PRODUCTION_PATHS {
        let source = fs::read_to_string(repository.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        let raw_literals = source
            .lines()
            .filter(|line| {
                line.contains("CapabilityOffer {")
                    && !line.contains("-> CapabilityOffer {")
                    && !line.contains("struct CapabilityOffer {")
            })
            .count();
        if raw_literals != *expected_raw_literals {
            violations.push(format!(
                "{relative}: expected {expected_raw_literals} reviewed raw literal(s) ({reason}), found {raw_literals}; update the ratchet when migrating debt"
            ));
        }
        if !source.contains("CapabilityOfferBuilder")
            && !source.contains("realization_offer")
            && !source.contains("SemanticCapabilityContract")
        {
            violations.push(format!(
                "{relative}: lost the canonical capability-offer construction path"
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "migrated production capability offers regressed:\n{}",
        violations.join("\n")
    );
}
