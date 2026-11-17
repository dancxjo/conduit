//! Checked Source and synthetic metadata tests; no physical playback claim.
use super::*;
use std::cell::Cell;

fn checked(speaker: &str) -> CheckedHostConfiguration {
    let source = include_str!("../../profiles/linux-computer.host.conduit")
        .replace("  limits =", &format!("{speaker}\n  limits ="));
    let packages = conduit_workspace_make::package_set();
    let catalog = conduit_workspace_make::catalog();
    conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap()
}
fn speaker() -> &'static str {
    r#"  base = {kind: "audio/speaker", implementation: "std/kernel-audio-play-alsa-hw@1", initialization: {executable_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", card_id: "vc4hdmi0", device: 0, base_identity: "hdmi-controller"}}"#
}
fn observation() -> AlsaPlaybackObservation {
    AlsaPlaybackObservation {
        card_index: 2,
        card_id: "vc4hdmi0".into(),
        card_name: "HDMI".into(),
        device: 0,
        device_name: "HDMI 0".into(),
        base_identity: "hdmi-controller".into(),
    }
}
#[test]
fn checked_declaration_selects_exact_observed_provider_and_preserves_boot_scope() {
    let config = checked(speaker());
    let verified = Cell::new(false);
    let initial = prepare_with(
        &config,
        "boot/current".into(),
        OfferGeneration(7),
        |digest| {
            assert_eq!(*digest, [0xaa; 32]);
            verified.set(true);
            Ok(())
        },
        || {
            assert!(verified.get());
            Ok(vec![observation()])
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(initial.selection.boot_id.as_str(), "boot/current");
    assert_eq!(initial.selection.offer_generation, OfferGeneration(7));
    assert_eq!(initial.selection.alsa_target(), "hw:CARD=vc4hdmi0,DEV=0");
    assert_eq!(initial.entry.mechanism_family.as_str(), "audio/speaker");
    assert_eq!(initial.entry.provider_generation, 7);
    assert_eq!(initial.entry.capabilities.len(), 1);
    assert!(!initial.entry.capabilities[0]
        .authority_requirements
        .is_empty());
    assert_eq!(initial.entry.resources[0].capacity, 1);
}
#[test]
fn missing_selection_performs_no_discovery_or_executable_access() {
    assert!(prepare_with(
        &checked(""),
        "boot/current".into(),
        OfferGeneration(1),
        |_| panic!("no declared provider"),
        || panic!("no declared device")
    )
    .unwrap()
    .is_none());
}
#[test]
fn changed_executable_refuses_before_any_metadata_command() {
    assert!(prepare_with(
        &checked(speaker()),
        "boot/current".into(),
        OfferGeneration(1),
        |_| Err("provider hash mismatch".into()),
        || panic!("must verify before invoking executable")
    )
    .is_err());
}
#[test]
fn absent_ambiguous_or_changed_physical_selector_refuses() {
    let config = checked(speaker());
    for rows in [vec![], vec![observation(), observation()]] {
        assert!(prepare_with(
            &config,
            "boot/current".into(),
            OfferGeneration(1),
            |_| Ok(()),
            || Ok(rows)
        )
        .is_err());
    }
    for field in 0..3 {
        let mut other = observation();
        match field {
            0 => other.card_id = "other".into(),
            1 => other.device = 1,
            _ => other.base_identity = "different-controller".into(),
        }
        assert!(prepare_with(
            &config,
            "boot/current".into(),
            OfferGeneration(1),
            |_| Ok(()),
            || Ok(vec![other])
        )
        .is_err());
    }
}
#[test]
fn malformed_digest_never_invokes_provider() {
    for digest in ["a".repeat(63), "A".repeat(64), "g".repeat(64)] {
        let source = speaker().replace(&"a".repeat(64), &digest);
        assert!(prepare_with(
            &checked(&source),
            "boot/current".into(),
            OfferGeneration(1),
            |_| panic!("malformed digest"),
            || panic!("malformed digest")
        )
        .is_err());
    }
}

#[test]
fn provider_drift_during_metadata_refuses_initialization() {
    let calls = Cell::new(0);
    let result = prepare_with(
        &checked(speaker()),
        "boot/current".into(),
        OfferGeneration(1),
        |_| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                Ok(())
            } else {
                Err("provider changed during discovery".into())
            }
        },
        || Ok(vec![observation()]),
    );
    assert!(result.is_err());
    assert_eq!(calls.get(), 2);
}
