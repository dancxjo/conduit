use crate::test_packages::{test_build_host_image, test_catalog};
use crate::*;

const STD_COMPUTER: &str = include_str!("../../../targets/std/profiles/std-computer.profile.json");
const CONDUITOS_NATIVE: &str =
    include_str!("../../../targets/conduitos/profiles/conduitos-native.profile.json");
const BROWSER_PAGE: &str =
    include_str!("../../../targets/browser/profiles/browser-page.profile.json");
const CONDUITOS_HEADLESS: &str =
    include_str!("../../../targets/conduitos/profiles/conduitos-headless.profile.json");
const CONDUITOS_AARCH64_HEADLESS: &str =
    include_str!("../../../targets/conduitos/profiles/conduitos-aarch64-headless.profile.json");

fn parse(source: &str) -> HostProfile {
    serde_json::from_str(source).unwrap()
}

#[test]
fn four_materially_different_checked_in_profiles_validate() {
    let catalog = test_catalog();
    let profiles = [
        parse(STD_COMPUTER),
        parse(CONDUITOS_NATIVE),
        parse(BROWSER_PAGE),
        parse(CONDUITOS_HEADLESS),
    ];
    let validated = profiles
        .into_iter()
        .map(|profile| validate_profile(profile, &catalog).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(validated.len(), 4);
    assert!(validated
        .iter()
        .any(|profile| profile.profile().presenters.is_empty()));
    assert!(validated.iter().any(|profile| {
        profile
            .profile()
            .facilities
            .contains(&"compositor/native@1".to_owned())
    }));
    assert!(validated
        .iter()
        .any(|profile| profile.profile().target.family == "browser"));
    assert_eq!(
        validated
            .iter()
            .map(|profile| profile.profile_id().as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
}

#[test]
fn aarch64_virt_profile_closes_the_exact_linear_serial_presenter() {
    let catalog = test_catalog();
    let profile = parse(CONDUITOS_AARCH64_HEADLESS);
    let validated = validate_profile(profile.clone(), &catalog).unwrap();
    assert_eq!(validated.profile().target.key(), "conduitos/aarch64/virt");
    for required in [
        "host-operation:conduit.host/present@1",
        "base:serial/text",
        "driver:conduitos/pl011@1",
    ] {
        assert!(validated
            .dependency_paths()
            .keys()
            .any(|path| path.contains(required)));
    }

    let mut missing_driver = profile.clone();
    missing_driver.drivers.clear();
    let diagnostics = validate_profile(missing_driver, &catalog).unwrap_err();
    assert!(diagnostics.iter().any(|item| matches!(
        item,
        ProfileDiagnostic::UnsatisfiedPrerequisite { missing, .. }
            if missing == "driver:conduitos/pl011@1"
    )));

    let mut wrong_target = profile;
    wrong_target.target.architecture = "x86_64".into();
    wrong_target.target.machine = "pc".into();
    let diagnostics = validate_profile(wrong_target, &catalog).unwrap_err();
    assert!(diagnostics.iter().any(|item| matches!(
        item,
        ProfileDiagnostic::TargetIncompatible { item, target }
            if item == "presenter/linear-serial@1" && target == "conduitos/x86_64/pc"
    )));
}

#[test]
fn canonical_identity_ignores_declaration_order_but_not_meaning() {
    let catalog = test_catalog();
    let profile = parse(STD_COMPUTER);
    let expected = validate_profile(profile.clone(), &catalog).unwrap();
    let mut reordered = profile.clone();
    reordered.exclusions.reverse();
    reordered.host_operations.reverse();
    reordered.resources.reverse();
    let actual = validate_profile(reordered, &catalog).unwrap();
    assert_eq!(expected.profile_id(), actual.profile_id());

    let mut changed = profile;
    changed.bounds.queue_items += 1;
    let changed = validate_profile(changed, &catalog).unwrap();
    assert_ne!(expected.profile_id(), changed.profile_id());
    assert!(expected.profile_id().as_str().starts_with("sha256:"));
}

#[test]
fn canonical_std_offer_metadata_drives_exact_prerequisites() {
    let catalog = test_catalog();
    let profile = parse(STD_COMPUTER);
    let validated = validate_profile(profile.clone(), &catalog).unwrap();
    assert!(validated.dependency_paths().keys().any(|path| {
        path.contains("capability:time/tick@conduit.std/time-tick@2")
            && path.contains("host-operation:conduit.host/wait@1")
    }));

    let mut missing = profile;
    missing.host_operations.clear();
    let diagnostics = validate_profile(missing, &catalog).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        ProfileDiagnostic::UnsatisfiedPrerequisite { requester, missing }
            if requester.contains("time/tick")
                && missing == "host-operation:conduit.host/wait@1"
    )));
}

#[test]
fn invalid_unknown_unbounded_and_contradictory_profiles_fail_specifically() {
    let catalog = test_catalog();
    let mut profile = parse(STD_COMPUTER);
    profile.target.machine = "unknown".into();
    profile.resources[0].slots = 0;
    profile.resources.push(profile.resources[0].clone());
    profile
        .exclusions
        .push("presenter/native-graphical@1".into());
    let diagnostics = validate_profile(profile, &catalog).unwrap_err();
    assert!(diagnostics.iter().any(|item| matches!(
        item,
        ProfileDiagnostic::UnknownReference {
            field: "target",
            ..
        }
    )));
    assert!(diagnostics
        .iter()
        .any(|item| matches!(item, ProfileDiagnostic::UnboundedResource { .. })));
    assert!(diagnostics.iter().any(|item| matches!(
        item,
        ProfileDiagnostic::DuplicateIdentity {
            field: "resource",
            ..
        }
    )));
    assert!(diagnostics
        .iter()
        .any(|item| matches!(item, ProfileDiagnostic::Contradiction { .. })));
}

#[test]
fn presenter_without_compositor_display_or_driver_fails_closed() {
    let catalog = test_catalog();
    let mut profile = parse(CONDUITOS_NATIVE);
    profile.facilities.clear();
    profile.bases.clear();
    profile.drivers.clear();
    let diagnostics = validate_profile(profile, &catalog).unwrap_err();
    for required in [
        "facility:compositor/native@1",
        "base:display/scanout",
        "driver:display/linear-framebuffer@1",
    ] {
        assert!(diagnostics.iter().any(|item| matches!(
            item,
            ProfileDiagnostic::UnsatisfiedPrerequisite { missing, .. } if missing == required
        )));
    }
}

#[test]
fn target_incompatible_base_and_driver_fail_before_target_lowering() {
    let catalog = test_catalog();
    let mut profile = parse(CONDUITOS_NATIVE);
    profile.target.family = "browser".into();
    profile.target.architecture = "wasm32".into();
    profile.target.machine = "page".into();
    profile.presenters.clear();
    let diagnostics = validate_profile(profile, &catalog).unwrap_err();
    for incompatible in ["display/scanout", "display/linear-framebuffer@1"] {
        assert!(diagnostics.iter().any(|item| matches!(
            item,
            ProfileDiagnostic::TargetIncompatible { item, target }
                if item == incompatible && target == "browser/wasm32/page"
        )));
    }
}

#[test]
fn circular_prerequisite_metadata_is_rejected() {
    let mut catalog = test_catalog();
    let facility = PrerequisiteNode::Facility("compositor/native@1".into());
    let base = PrerequisiteNode::Base("display/scanout".into());
    catalog
        .dependencies
        .entry(facility.clone())
        .or_default()
        .push(base.clone());
    catalog.dependencies.entry(base).or_default().push(facility);
    let diagnostics = validate_profile(parse(CONDUITOS_NATIVE), &catalog).unwrap_err();
    assert!(diagnostics.iter().any(
        |item| matches!(item, ProfileDiagnostic::CircularPrerequisite { path } if path.len() >= 3)
    ));
}

#[test]
fn profile_validation_is_inert_machinery_description() {
    let validated = validate_profile(parse(CONDUITOS_HEADLESS), &test_catalog()).unwrap();
    let debug = format!("{validated:?}");
    for runtime_truth in [
        "HostAdvertisement",
        "HostOffer",
        "BodyId",
        "PlanId",
        "ActivePlayId",
    ] {
        assert!(!debug.contains(runtime_truth));
    }
}

fn build_inputs() -> BuildInputs {
    BuildInputs {
        source_identity: "git:a467ae61".into(),
        toolchain_available: true,
    }
}

#[test]
fn three_profiles_build_through_one_deterministic_pipeline() {
    let catalog = test_catalog();
    let profiles = [
        parse(STD_COMPUTER),
        parse(BROWSER_PAGE),
        parse(CONDUITOS_HEADLESS),
    ];
    let images = profiles
        .into_iter()
        .map(|profile| {
            let first = test_build_host_image(profile.clone(), &catalog, &build_inputs()).unwrap();
            let second = test_build_host_image(profile, &catalog, &build_inputs()).unwrap();
            assert_eq!(first, second);
            verify_image_binding(&first.0, &first.1).unwrap();
            first.0
        })
        .collect::<Vec<_>>();
    assert_eq!(
        images[0].manifest.post_build_actions,
        [PostBuildAction::Launch]
    );
    assert_eq!(
        images[1].manifest.post_build_actions,
        [PostBuildAction::Load, PostBuildAction::Launch]
    );
    assert_eq!(
        images[2].manifest.post_build_actions,
        [PostBuildAction::Boot]
    );
}

#[test]
fn build_identity_uses_canonical_profile_meaning_not_declaration_order() {
    let catalog = test_catalog();
    let profile = parse(STD_COMPUTER);
    let expected = test_build_host_image(profile.clone(), &catalog, &build_inputs()).unwrap();
    let mut reordered = profile;
    reordered.host_operations.reverse();
    reordered.resources.reverse();
    reordered.presenters.reverse();
    let actual = test_build_host_image(reordered, &catalog, &build_inputs()).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn profile_controls_graphical_inclusion_and_headless_omission() {
    let catalog = test_catalog();
    let graphical = test_build_host_image(parse(CONDUITOS_NATIVE), &catalog, &build_inputs())
        .unwrap()
        .0;
    assert!(graphical
        .manifest
        .presenters
        .contains(&"presenter/native-graphical@1".into()));
    assert!(graphical
        .manifest
        .facilities
        .contains(&"compositor/native@1".into()));
    assert!(graphical
        .manifest
        .inclusion_paths
        .keys()
        .any(|path| path.contains("presenter:presenter/main")));

    let headless = test_build_host_image(parse(CONDUITOS_HEADLESS), &catalog, &build_inputs())
        .unwrap()
        .0;
    assert!(headless.manifest.presenters.is_empty());
    assert!(headless.manifest.facilities.is_empty());
}

#[test]
fn build_refuses_budget_toolchain_and_artifact_binding_failures() {
    let catalog = test_catalog();
    let mut profile = parse(STD_COMPUTER);
    profile.bounds.queue_items = u32::MAX;
    let mut inputs = build_inputs();
    inputs.toolchain_available = false;
    let diagnostics = test_build_host_image(profile.clone(), &catalog, &inputs).unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|item| matches!(item, BuildDiagnostic::ToolchainUnavailable { .. })));
    assert!(diagnostics.iter().any(|item| matches!(
        item,
        BuildDiagnostic::ResourceBudgetOverflow {
            field: "queue_items",
            ..
        }
    )));

    let (image, mut bytes) =
        test_build_host_image(parse(STD_COMPUTER), &catalog, &build_inputs()).unwrap();
    bytes[0] = b'[';
    assert!(matches!(
        verify_image_binding(&image, &bytes),
        Err(BuildDiagnostic::Encoding { .. })
            | Err(BuildDiagnostic::ArtifactBindingMismatch { .. })
    ));
}

#[test]
fn build_output_contains_no_runtime_truth() {
    let (image, _) =
        test_build_host_image(parse(STD_COMPUTER), &test_catalog(), &build_inputs()).unwrap();
    let encoded = serde_json::to_string(&image).unwrap();
    for forbidden in ["BodyId", "PlanId", "PlayId", "HostOffer", "BootId"] {
        assert!(!encoded.contains(forbidden));
    }
}

fn tick_runtime_facts(timer_ready: bool) -> RuntimeFacts {
    RuntimeFacts {
        ready_resource_classes: timer_ready
            .then(|| "conduit.resource/timer-slot@1".to_owned())
            .into_iter()
            .collect(),
        initialized_base_kinds: timer_ready
            .then(|| "timer/monotonic".to_owned())
            .into_iter()
            .collect(),
        initialized_driver_kinds: timer_ready
            .then(|| "hosted/monotonic-clock@1".to_owned())
            .into_iter()
            .collect(),
        available_facilities: Default::default(),
        authority_ready: false,
    }
}

fn bind_tick_runtime(
    image: &HostImage,
    bytes: &[u8],
    boot: &str,
    generation: u64,
    timer_ready: bool,
) -> BoundHostAdvertisement {
    bind_runtime_offer(
        &image.manifest,
        image,
        bytes,
        &test_catalog(),
        RuntimeOfferInputs {
            host_id: conduit_core::HostId::from("host/durable"),
            boot_id: conduit_core::BootId::from(boot),
            offer_generation: conduit_core::OfferGeneration(generation),
            offer_sign_id: conduit_core::SignId::from(format!("{boot}/offer/{generation}")),
            host_profile: conduit_core::HostProfileId::from("std/runtime-bound@1"),
            candidate_resources: vec![
                conduit_core::resource_offer(
                    "fabrication-test/presentation",
                    conduit_core::PRESENTATION_RESOURCE_CLASS,
                    16,
                ),
                conduit_core::resource_offer(
                    "fabrication-test/timer",
                    conduit_core::TIMER_RESOURCE_CLASS,
                    16,
                ),
            ],
            candidate_capabilities: vec![conduit_std_offers::tick_capability_offer()],
            planner_capabilities: Vec::new(),
            facts: tick_runtime_facts(timer_ready),
        },
    )
    .unwrap()
}

#[test]
fn compiled_capability_is_absent_until_runtime_prerequisites_are_ready() {
    let (image, bytes) =
        test_build_host_image(parse(STD_COMPUTER), &test_catalog(), &build_inputs()).unwrap();
    let unavailable = bind_tick_runtime(&image, &bytes, "boot/a", 1, false);
    assert!(unavailable.advertisement().capabilities.is_empty());
    assert!(unavailable.advertisement().resources.is_empty());
    assert_eq!(
        unavailable.identity().offer_sign_id.as_str(),
        "boot/a/offer/1"
    );

    let available = bind_tick_runtime(&image, &bytes, "boot/a", 2, true);
    assert_eq!(available.advertisement().capabilities.len(), 1);
    assert_eq!(available.advertisement().offer_generation.0, 2);
    assert_eq!(
        available.identity().offer_sign_id.as_str(),
        "boot/a/offer/2"
    );
    assert_eq!(available.identity().image_id, image.manifest.image_id);
}

#[test]
fn runtime_cannot_advertise_unbuilt_implementation_or_stale_boot() {
    let (image, bytes) =
        test_build_host_image(parse(STD_COMPUTER), &test_catalog(), &build_inputs()).unwrap();
    let bound = bind_tick_runtime(&image, &bytes, "boot/current", 1, true);
    let (identity, mut advertisement) = bound.into_parts();
    advertisement.capabilities[0]
        .implementation
        .implementation_id = conduit_core::ImplementationId::from("runtime/invented@1");
    assert!(matches!(
        verify_runtime_advertisement(&identity, &advertisement, &image.manifest, &image, &bytes),
        Err(RuntimeBindingDiagnostic::UnexpectedImplementation { .. })
    ));

    advertisement.capabilities[0] = conduit_std_offers::tick_capability_offer();
    advertisement.boot_id = conduit_core::BootId::from("boot/stale");
    assert_eq!(
        verify_runtime_advertisement(&identity, &advertisement, &image.manifest, &image, &bytes),
        Err(RuntimeBindingDiagnostic::IdentityMismatch { field: "boot_id" })
    );
}

#[test]
fn rebuild_reseals_image_while_old_boot_truth_remains_old() {
    let catalog = test_catalog();
    let profile = parse(STD_COMPUTER);
    let (old_image, old_bytes) =
        test_build_host_image(profile.clone(), &catalog, &build_inputs()).unwrap();
    let old = bind_tick_runtime(&old_image, &old_bytes, "boot/old", 1, true);

    let mut changed = profile;
    changed.bounds.timer_slots += 1;
    let (new_image, new_bytes) = test_build_host_image(changed, &catalog, &build_inputs()).unwrap();
    let new = bind_tick_runtime(&new_image, &new_bytes, "boot/new", 1, true);
    assert_ne!(old.identity().profile_id, new.identity().profile_id);
    assert_ne!(old.identity().build_id, new.identity().build_id);
    assert_ne!(old.identity().image_id, new.identity().image_id);
    assert_ne!(old.identity().boot_id, new.identity().boot_id);
    assert_eq!(old.advertisement().boot_id.as_str(), "boot/old");
    assert!(matches!(
        verify_bound_advertisement(&old, &new_image.manifest, &new_image, &new_bytes),
        Err(RuntimeBindingDiagnostic::IdentityMismatch { .. })
    ));
}
