use std::collections::BTreeSet;

use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration};
use conduit_host_make::{
    bind_runtime_offer, build_default_host_image, check_host_configuration,
    parse_host_configuration_conduit, BuildInputs, HostImage, RuntimeFacts, RuntimeOfferInputs,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

use crate::{StdHost, TimerAdapter};

fn bound_host(source: &str, boot: &str, extra_timer_slots: u32) -> (StdHost, HostImage) {
    // Construction includes a display, but this fixture initializes only its
    // timer. Resolve the complete finite Mask prerequisites through the same
    // checked Source entrance as ordinary Host construction.
    let configuration_source = r#"host std-image-binding {
  schema = 1
  target = {architecture: "x86_64", machine: "computer", os: "linux"}
  base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
  base = {kind: "display/scanout", implementation: "display/linear-framebuffer@1"}
  capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
  need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 128, bytes: 8192}
  need = {id: "display/main", class: "face/surface", slots: 8, bytes: 8388608}
  need = {id: "display/memory", class: "conduit.resource/runtime-memory@1", slots: 1048576, bytes: 1048576}
  need = {id: "display/show", class: "face.resource/show@1", slots: 1, bytes: 1}
  limits = {static_memory_bytes: 16777216, heap_arena_bytes: 67108864, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}
}"#;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(configuration_source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let mut profile = checked.profile().clone();
    profile.bounds.timer_slots += extra_timer_slots;
    let inputs = BuildInputs {
        source_identity: source.into(),
        toolchain_available: true,
    };
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
            candidate_bases: vec![conduit_core::BaseProviderEntry {
                base_id: conduit_core::HostBaseId::from("std/timer/0"),
                provider_instance_id: conduit_core::BaseInstanceId::from(format!(
                    "{boot}/timer/provider/1"
                )),
                provider_generation: 1,
                implementation_id: conduit_core::BaseImplementationId::from(
                    "hosted/monotonic-clock@1",
                ),
                mechanism_family: conduit_core::HostBaseKindId::from("timer/monotonic"),
                enforcement_class: conduit_core::BaseEnforcementClass::Cooperative,
                lifecycle: conduit_core::BaseLifecycle::Ready,
                capabilities: vec![conduit_std_offers::tick_capability_offer()],
                resources: vec![conduit_core::resource_offer(
                    "std-image-test/timer",
                    conduit_core::TIMER_RESOURCE_CLASS,
                    16,
                )],
            }],
            candidate_capabilities: vec![],
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
    assert!(image
        .payload
        .implementations
        .iter()
        .any(|implementation| { implementation == "face/renderer-conduitos-native@1" }));
    assert_eq!(host.advertisement().capabilities.len(), 1);
    assert_eq!(
        host.advertisement().capabilities[0].kind_id.as_str(),
        "time/tick"
    );
}

#[derive(Default)]
struct NoopTimer;

impl TimerAdapter for NoopTimer {
    fn wait(&mut self, _duration: std::time::Duration) {}
}

#[test]
fn rebuild_keeps_the_old_plan_bound_to_the_old_boot() {
    let (old_host, old_image) = bound_host("git:old", "std/boot-old", 0);
    let source = "plot clock-demo {\n    clock: time/tick(count = 1, period-ms = 0)\n}\n";
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "clock-demo", &profile).unwrap();
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
