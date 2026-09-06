use std::collections::BTreeSet;

use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_host_fabrication::{
    bind_runtime_offer, build_default_host_image, BuildInputs, HostImage, HostProfile,
    RuntimeFacts, RuntimeOfferInputs,
};

use crate::{StdHost, TimerAdapter};

fn bound_host(source: &str, boot: &str, extra_timer_slots: u32) -> (StdHost, HostImage) {
    let mut profile: HostProfile =
        serde_json::from_str(include_str!("../profiles/std-computer.profile.json")).unwrap();
    profile.bounds.timer_slots += extra_timer_slots;
    let inputs = BuildInputs {
        source_identity: source.into(),
        toolchain_available: true,
    };
    let catalog = conduit_workspace_fabrication::catalog();
    let packages = conduit_workspace_fabrication::package_set();
    let (image, bytes) = build_default_host_image(profile, &catalog, &packages, &inputs).unwrap();
    let binding = bind_runtime_offer(
        &image.manifest,
        &image,
        &bytes,
        &catalog,
        RuntimeOfferInputs {
            host_id: HostId::from("std/durable"),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(1),
            offer_sign_id: conduit_core::SignId::from(format!("sign/{boot}/ready/1")),
            host_profile: HostProfileId::from("std/image-bound@1"),
            candidate_resources: vec![conduit_core::resource_offer(
                "std-image-test/timer",
                conduit_core::TIMER_RESOURCE_CLASS,
                16,
            )],
            candidate_capabilities: vec![conduit_std_offers::tick_capability_offer()],
            planner_capabilities: Vec::new(),
            facts: RuntimeFacts {
                ready_resource_classes: BTreeSet::from(["conduit.resource/timer-slot@1".into()]),
                initialized_base_kinds: BTreeSet::from(["timer/monotonic".into()]),
                initialized_driver_kinds: BTreeSet::from(["hosted/monotonic-clock@1".into()]),
                available_facilities: BTreeSet::new(),
                authority_ready: false,
            },
        },
    )
    .unwrap();
    (StdHost::from_image_binding(binding).unwrap(), image)
}

#[test]
fn running_std_host_exposes_exact_image_identity_and_truthful_offer_subset() {
    let (host, image) = bound_host("git:std-host-test", "std/boot-exact", 0);
    let identity = host.image_identity().unwrap();
    assert_eq!(identity.profile_id, image.manifest.profile_id);
    assert_eq!(identity.build_id, image.manifest.build_id);
    assert_eq!(identity.image_id, image.manifest.image_id);
    assert_eq!(identity.boot_id.as_str(), "std/boot-exact");
    assert_eq!(host.advertisement().capabilities.len(), 1);
}

#[derive(Default)]
struct NoopTimer;

impl TimerAdapter for NoopTimer {
    fn wait(&mut self, _duration: std::time::Duration) {}
}

#[test]
fn rebuild_keeps_the_old_plan_bound_to_the_old_boot() {
    let (old_host, old_image) = bound_host("git:old", "std/boot-old", 0);
    let source = "form clock-demo {\n    clock: time/tick(count = 1, period-ms = 0)\n}\n";
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "clock-demo", &profile).unwrap();
    let old_plan = old_host.plan_expanded_local(&expanded).unwrap();
    assert_eq!(old_plan.fragments[0].boot_id.as_str(), "std/boot-old");

    let (mut rebuilt_host, rebuilt_image) = bound_host("git:new", "std/boot-new", 1);
    assert_ne!(old_image.manifest.image_id, rebuilt_image.manifest.image_id);
    assert_eq!(old_plan.fragments[0].boot_id.as_str(), "std/boot-old");
    let mut output = Vec::new();
    assert!(rebuilt_host
        .run_fragment_to(old_plan.fragments[0].clone(), &mut output, &mut NoopTimer)
        .is_err());
}
