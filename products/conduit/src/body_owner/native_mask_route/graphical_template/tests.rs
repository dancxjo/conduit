//! CPU preparation conformance, not signed membership or physical effects.
use super::*;

fn advertisement(name: &str) -> HostAdvertisement {
    let mut provider = BaseProviderEntry {
        base_id: format!("{name}/framebuffer").into(),
        provider_instance_id: format!("{name}/framebuffer/1").into(),
        provider_generation: 2,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![
            resource_offer(
                &format!("{name}/surface"),
                conduit_face::SHOW_RESOURCE_CLASS,
                1,
            ),
            resource_offer(
                &format!("{name}/input"),
                "conduit.resource/conduitos-human-input@1",
                1,
            ),
        ],
    };
    provider.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let mut ad = conduit_conduitos_mask_offer::native_host_advertisement(
        &name.into(),
        &format!("{name}/boot").into(),
        7,
        &provider,
    );
    provider.capabilities = ad
        .capabilities
        .iter()
        .filter(|c| {
            [
                conduit_face::SHOW_RESOURCE_SOURCE_KIND,
                conduit_face::RESOURCE_RENDERER_KIND,
                conduit_face::FACE_INTERACTION_KIND,
            ]
            .contains(&c.kind_id.as_str())
        })
        .cloned()
        .collect();
    ad.capabilities.retain(|c| {
        c.kind_id.as_str() == conduit_face::FACE_TEE_KIND || provider.capabilities.contains(c)
    });
    ad.resources = provider.resources;
    ad.bases[0].capability_ids = provider
        .capabilities
        .iter()
        .map(|c| c.capability_id.clone())
        .collect();
    ad
}
fn lines(owner: &HostAdvertisement, member: &HostAdvertisement) -> [LineOffer; 3] {
    std::array::from_fn(|i| {
        let (a, b) = if i == 0 {
            (owner, member)
        } else {
            (member, owner)
        };
        let mut line = process_owned_line_offer_with_limits(
            &format!("{}/line/{i}", member.host_id.as_str()),
            &format!("{}/binding/{i}", member.host_id.as_str()),
            "conduit.base/websocket-rfc6455@1".into(),
            &format!("{}/socket/{i}", member.host_id.as_str()),
            a,
            b,
            LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64 * 1024,
                maximum_buffered_bytes: 128 * 1024,
                maximum_frame_bytes: 64 * 1024,
            },
        );
        // This fixture models a routed authenticated member transport. The
        // process-owned helper alone is explicitly not a cross-Host Fore Line.
        line.contract.scope = LineScope::RoutedNetwork;
        line.contract.security = LineSecurity::AuthenticatedEncrypted;
        line.binding.credential = LinkCredentialReference::Opaque(format!("credential/{i}").into());
        line.binding.authority = LinkAuthorityReference::Grant(format!("grant/{i}").into());
        line
    })
}
fn ordinary(
    ad: &HostAdvertisement,
    owner: &HostAdvertisement,
    lines: &[LineOffer; 3],
) -> PlannedMaskPlot {
    super::super::graphical_plan::plan(ad, owner, None, &lines[0], &lines[1], Some(&lines[2]))
        .unwrap()
}
#[test]
fn fixed_graphical_template_matches_ordinary_target_plan_and_funds_result() {
    let owner = advertisement("host/owner");
    let target = advertisement("host/target-with-different-length");
    let canonical =
        conduit_conduitos_mask_offer::canonical_native_graphical_template_preparation().construct();
    let seal = canonical.planned_mask();
    assert_eq!(
        conduit_conduitos_mask_offer::canonical_native_graphical_template_preparation()
            .retained_bytes(),
        size_of::<conduit_conduitos_mask_offer::CanonicalNativeGraphicalTemplate>()
            + conduit_std_host::native_mask_plan_backing_bytes(seal).unwrap()
    );

    let lines = lines(&owner, &target);
    let checked = preflight(seal, &target, &owner, [&lines[0], &lines[1], &lines[2]])
        .expect("closed preflight");
    let result = build(seal, &checked, &target).unwrap();
    assert_eq!(result, ordinary(&target, &owner, &lines));
    let actual = conduit_std_host::native_mask_plan_backing_bytes(&result).unwrap()
        + size_of::<PlannedMaskPlot>();
    assert!(actual <= checked.retained && checked.retained < checked.peak);
    assert!(verify_plan(&result.plan));
}
#[test]
fn fixed_graphical_template_refuses_changed_closure_ambiguous_pool_and_stale_endpoints() {
    let owner = advertisement("host/owner");
    let target = advertisement("host/target");
    let canonical =
        conduit_conduitos_mask_offer::canonical_native_graphical_template_preparation().construct();
    let seal = canonical.planned_mask();
    let lines = lines(&owner, &target);
    let check = |ad: &HostAdvertisement, ls: &[LineOffer; 3]| {
        preflight(seal, ad, &owner, [&ls[0], &ls[1], &ls[2]]).is_some()
    };
    assert!(check(&target, &lines));
    let mut changed = target.clone();
    changed.capabilities[0].limits.max_queue_bytes += 1;
    assert!(!check(&changed, &lines));
    let mut changed = target.clone();
    let mut duplicate = changed.resources[0].clone();
    duplicate.pool_id = "other/pool".into();
    changed.resources.push(duplicate);
    assert!(!check(&changed, &lines));
    let mut process = lines.clone();
    process[0].contract.scope = LineScope::Process;
    assert!(!check(&target, &process));
    let mut stale = lines.clone();
    stale[0].binding.sink.boot_id = "old/boot".into();
    assert!(!check(&target, &stale));
    let mut grant = seal.clone();
    grant.plan.fragments[0]
        .placements
        .iter_mut()
        .flat_map(|g| &mut g.authority)
        .next()
        .unwrap()
        .grant_id = "foreign/grant".into();
    assert!(preflight(&grant, &target, &owner, [&lines[0], &lines[1], &lines[2]]).is_none());
}
