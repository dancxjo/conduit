//! Actual SDL dummy/software lifetime diagnostics, never physical DRM proof.
use super::*;
use conduit_linux_display::Scanout;

#[test]
fn actual_initialized_lifetime_admits_atomically_and_retracts_only_own_offers() {
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    assert_eq!(video.current_video_driver(), "dummy");
    let window = video
        .window("Host readiness diagnostic", 640, 480)
        .hidden()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut scanout = Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
    let token = scanout.initialized_display().unwrap();
    let old_nonce = token.instance_nonce();
    let (mut host, _, _) = crate::host_execution::scalar_tape_fixture::prepare(
        "plot arithmetic (\n >> value: U64\n result: U64 >>\n) = (. + 1)\n",
        "arithmetic",
        true,
    );
    let unrelated = BaseProviderEntry {
        base_id: "diagnostic/unrelated-base".into(),
        provider_instance_id: "diagnostic/unrelated-instance".into(),
        provider_generation: 1,
        implementation_id: "diagnostic/unrelated@1".into(),
        mechanism_family: "diagnostic/base@1".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![conduit_core::resource_offer(
            "diagnostic/unrelated-pool",
            "diagnostic/resource@1",
            1,
        )],
    };
    host.base_registry.register(unrelated.clone()).unwrap();
    host.base_registry
        .project_base_ready_into(&unrelated.base_id, &mut host.advertisement)
        .unwrap();
    host.advertisement.resources.sort();
    host.kernel_resources = KernelResourceLedger::new(&host.advertisement).unwrap();
    let before = host.advertisement.clone();
    assert!(matches!(
        host.attach_initialized_display(token.clone()),
        Err(DisplayAttachmentRefusal::UnsupportedBackend)
    ));
    assert_eq!(
        host.advertisement, before,
        "public API cannot admit diagnostic SDL"
    );
    assert!(host.native_display.is_none());

    let idle_plan = plan_arithmetic(&host);
    let reservation = host
        .kernel_resources
        .prepare_and_reserve(&before, &idle_plan.fragments[0])
        .unwrap();
    assert!(matches!(
        host.attach_display(token.clone(), DisplayPolicy::DiagnosticDummy),
        Err(DisplayAttachmentRefusal::HostActive)
    ));
    assert_eq!(host.advertisement, before);
    host.kernel_resources.release(reservation).unwrap();

    let ready = host
        .attach_display(token.clone(), DisplayPolicy::DiagnosticDummy)
        .unwrap();
    let display_entry = ready.entry().clone();
    assert_eq!(ready.geometry(), (640, 480));
    assert_eq!(ready.resource().capacity_units, 1);
    assert_eq!(
        ready.resource().class_id.as_str(),
        conduit_face::SHOW_RESOURCE_CLASS
    );
    assert!(
        ready.entry().capabilities.is_empty(),
        "display observation mints no Mask capability"
    );
    assert_eq!(
        host.advertisement.offer_generation.0,
        before.offer_generation.0 + 1
    );
    assert_eq!(host.advertisement.capabilities, before.capabilities);
    assert_eq!(host.advertisement.bases.len(), before.bases.len() + 1);
    assert!(host.ready_native_display().is_some());
    assert!(!host.refresh_native_display().unwrap());
    let attached = host.advertisement.clone();
    assert!(matches!(
        host.attach_display(token.clone(), DisplayPolicy::DiagnosticDummy),
        Err(DisplayAttachmentRefusal::ExistingAttachment)
    ));
    assert_eq!(host.advertisement, attached);

    // A descriptor copied into another Boot or with altered resource truth is
    // insufficient for a borrowed readiness observation.
    let boot = host.advertisement.boot_id.clone();
    host.advertisement.boot_id = "diagnostic/other-boot".into();
    assert!(host.ready_native_display().is_none());
    host.advertisement.boot_id = boot;
    let resource = host
        .advertisement
        .resources
        .iter_mut()
        .find(|r| r.pool_id == display_entry.resources[0].pool_id)
        .unwrap();
    resource.capacity_units = 2;
    assert!(host.ready_native_display().is_none());
    *host
        .advertisement
        .resources
        .iter_mut()
        .find(|r| r.pool_id == display_entry.resources[0].pool_id)
        .unwrap() = display_entry.resources[0].clone();
    assert!(host.ready_native_display().is_some());

    // The same admitted display stays exclusively claimed while a real
    // ordinary checked Plan reservation is active on its owning Host.
    let attached_active_ad = host.advertisement.clone();
    let active_plan = plan_arithmetic(&host);
    let reservation = host
        .kernel_resources
        .prepare_and_reserve(&attached_active_ad, &active_plan.fragments[0])
        .unwrap();
    assert!(!host.kernel_resources.is_idle());
    let attached_active_registry = host.base_registry.clone();

    let mut second = StdHost::new();
    let second_ad = second.advertisement.clone();
    let second_registry = second.base_registry.clone();
    assert!(matches!(
        second.attach_display(token.clone(), DisplayPolicy::DiagnosticDummy),
        Err(DisplayAttachmentRefusal::DisplayAlreadyClaimed)
    ));
    assert_eq!(second.advertisement, second_ad);
    assert_eq!(second.base_registry, second_registry);
    assert!(second.native_display.is_none());
    assert!(second.kernel_resources.is_idle());
    assert!(
        token.is_live(),
        "rejected second claim cannot revoke the actual owner"
    );

    // Offer generation exhaustion refuses before committing any staged state.
    let mut exhausted = StdHost::new();
    exhausted.advertisement.offer_generation = OfferGeneration(u64::MAX);
    exhausted.kernel_resources = KernelResourceLedger::new(&exhausted.advertisement).unwrap();
    let exhausted_ad = exhausted.advertisement.clone();
    let exhausted_registry = exhausted.base_registry.clone();
    assert!(matches!(
        exhausted.attach_display(token.clone(), DisplayPolicy::DiagnosticDummy),
        Err(DisplayAttachmentRefusal::GenerationExhausted)
    ));
    assert_eq!(exhausted.advertisement, exhausted_ad);
    assert_eq!(exhausted.base_registry, exhausted_registry);
    assert!(exhausted.native_display.is_none());
    assert!(exhausted.kernel_resources.is_idle());

    scanout.lose();
    assert_eq!(
        host.refresh_native_display(),
        Err(DisplayAttachmentRefusal::HostActive)
    );
    assert_eq!(host.advertisement, attached_active_ad);
    assert_eq!(host.base_registry, attached_active_registry);
    assert!(
        !host.kernel_resources.is_idle(),
        "loss cannot replace the active ledger"
    );
    host.kernel_resources.release(reservation).unwrap();
    assert!(host.kernel_resources.is_idle());
    assert!(!token.is_live());
    assert!(host.ready_native_display().is_none());
    assert!(matches!(
        host.attach_display(token.clone(), DisplayPolicy::DiagnosticDummy),
        Err(DisplayAttachmentRefusal::DisplayLost)
    ));
    host.advertisement
        .resources
        .iter_mut()
        .find(|resource| resource.pool_id == display_entry.resources[0].pool_id)
        .unwrap()
        .capacity_units = 2;
    let changed = host.advertisement.clone();
    let registry = host.base_registry.clone();
    assert_eq!(
        host.refresh_native_display(),
        Err(DisplayAttachmentRefusal::BindingChanged)
    );
    assert_eq!(host.advertisement, changed);
    assert_eq!(host.base_registry, registry);
    *host
        .advertisement
        .resources
        .iter_mut()
        .find(|resource| resource.pool_id == display_entry.resources[0].pool_id)
        .unwrap() = display_entry.resources[0].clone();
    assert!(host.refresh_native_display().unwrap());
    assert_eq!(
        host.advertisement.offer_generation.0,
        attached.offer_generation.0 + 1
    );
    assert_eq!(host.advertisement.resources, before.resources);
    assert_eq!(host.advertisement.capabilities, before.capabilities);
    assert_eq!(host.advertisement.bases, before.bases);
    assert!(host.base_registry.entries().contains(&unrelated));
    assert_eq!(
        host.base_registry
            .entries()
            .iter()
            .find(|entry| entry.base_id == display_entry.base_id)
            .unwrap()
            .lifecycle,
        BaseLifecycle::Lost
    );
    assert!(!host.refresh_native_display().unwrap());
    assert!(matches!(
        host.attach_display(token, DisplayPolicy::DiagnosticDummy),
        Err(DisplayAttachmentRefusal::DisplayLost)
    ));
    drop(scanout);
    let fresh = Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
    let fresh_token = fresh.initialized_display().unwrap();
    assert_ne!(fresh_token.instance_nonce(), old_nonce);
    host.attach_display(fresh_token.clone(), DisplayPolicy::DiagnosticDummy)
        .unwrap();
    assert_ne!(
        host.ready_native_display().unwrap().entry().base_id,
        display_entry.base_id
    );
    assert_eq!(
        host.base_registry.entries().len(),
        3,
        "lost instance remains truthful bounded history"
    );
    drop(host);
    assert!(
        !fresh_token.is_live(),
        "dropping the admitted Host terminally revokes that actual display"
    );
    assert!(fresh.initialized_display().is_err());
    assert!(matches!(
        second.attach_display(fresh_token, DisplayPolicy::DiagnosticDummy),
        Err(DisplayAttachmentRefusal::DisplayLost)
    ));
    drop(fresh);
    let replacement = Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
    second
        .attach_display(
            replacement.initialized_display().unwrap(),
            DisplayPolicy::DiagnosticDummy,
        )
        .unwrap();
    assert!(second.ready_native_display().is_some());
    let entry = second.ready_native_display().unwrap().entry().clone();
    let prior_generation = second.advertisement.offer_generation.0;
    second
        .attach_creche_encounter(conduit_core::resource_offer(
            "diagnostic/creche-slot",
            conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
            1,
        ))
        .unwrap();
    assert_eq!(
        second.advertisement.offer_generation.0,
        prior_generation + 1
    );
    assert_eq!(second.ready_native_display().unwrap().entry(), &entry);
    assert!(replacement.initialized_display().unwrap().is_live());
}

fn plan_arithmetic(host: &StdHost) -> conduit_core::Plan {
    use conduit_core::{BaseImplementationId, ConnectionTrack, PortDirection};
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };
    use std::collections::BTreeMap;
    let checked = check_syntax_document(
        &parse_syntax_document("plot arithmetic (\n >> value: U64\n result: U64 >>\n) = (. + 1)\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "arithmetic", &ProfileCatalog::new())
            .unwrap();
    let hosts = [host.advertisement.clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4096,
            },
        )
    })
    .collect();
    conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap()
}
