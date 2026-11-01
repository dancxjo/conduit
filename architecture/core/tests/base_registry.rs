use conduit_core::{
    BaseEnforcementClass, BaseImplementationId, BaseInstanceId, BaseLifecycle, BaseProviderEntry,
    BaseRegistry, BaseRegistryLimits, BaseRegistryRefusal, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, HostBaseId, HostBaseKindId, HostId, HostProfileId, ImplementationOffer,
    KindId, KindIdentity, ThinHostSupervisor,
};

fn limits() -> BaseRegistryLimits {
    BaseRegistryLimits {
        maximum_bases: 2,
        maximum_capabilities_per_base: 1,
        maximum_resources_per_base: 1,
        maximum_advertised_capabilities: 3,
        maximum_advertised_resources: 2,
    }
}

fn capability(id: &str) -> CapabilityOffer {
    conduit_core::capability_offer_from_parts! {
        semantic_contract: Default::default(),
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(id),
        kind_id: KindId::from(id),
        kind_contract_revision: KindIdentity::from("revision"),
        inputs: vec![],
        outputs: vec![],
        implementation: ImplementationOffer {
            implementation_id: conduit_core::ImplementationId::from(format!("impl/{id}")),
            artifact_id: conduit_core::ArtifactId::from(format!("artifact/{id}")),
            execution_profile_id: conduit_core::ExecutionProfileId::from("pure-or-base"),
        },
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
    }
}

fn base(id: &str, generation: u64, capability_id: &str) -> BaseProviderEntry {
    BaseProviderEntry {
        base_id: HostBaseId::from(id),
        provider_instance_id: BaseInstanceId::from(format!("{id}/provider/{generation}")),
        provider_generation: generation,
        implementation_id: BaseImplementationId::from(format!("{id}/implementation@1")),
        mechanism_family: HostBaseKindId::from(format!("{id}/family")),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![capability(capability_id)],
        resources: vec![],
    }
}

fn supervisor() -> ThinHostSupervisor {
    ThinHostSupervisor::new(
        HostId::from("host/thin"),
        BootId::from("boot/current"),
        HostProfileId::from("profile/thin"),
        BaseRegistry::new(limits()).unwrap(),
    )
    .unwrap()
}

#[test]
fn empty_effect_host_advertises_only_pure_work() {
    let mut host = supervisor();
    host.set_pure_capabilities(vec![capability("text/upper")])
        .unwrap();

    let advertisement = host.advertisement().unwrap();
    assert_eq!(advertisement.host_id.as_str(), "host/thin");
    assert_eq!(advertisement.boot_id.as_str(), "boot/current");
    assert!(advertisement.resources.is_empty());
    assert_eq!(advertisement.capabilities.len(), 1);
    assert!(advertisement.bases.is_empty());
    assert_eq!(
        advertisement.capabilities[0].capability_id.as_str(),
        "text/upper"
    );
}

#[test]
fn independent_bases_aggregate_and_fail_independently() {
    let mut host = supervisor();
    host.registry_mut()
        .register(base("base/files", 1, "file/copy"))
        .unwrap();
    host.registry_mut()
        .register(base("base/network", 1, "http/request"))
        .unwrap();
    assert_eq!(host.advertisement().unwrap().capabilities.len(), 2);
    let advertised = host.advertisement().unwrap();
    assert_eq!(advertised.bases.len(), 2);
    assert!(advertised.bases.iter().any(|provider| {
        provider.base_id.as_str() == "base/files"
            && provider.provider_instance_id.as_str() == "base/files/provider/1"
            && provider.capability_ids[0].as_str() == "file/copy"
    }));

    host.registry_mut()
        .set_lifecycle(
            &HostBaseId::from("base/files"),
            &BaseInstanceId::from("base/files/provider/1"),
            1,
            BaseLifecycle::Lost,
        )
        .unwrap();
    let advertisement = host.advertisement().unwrap();
    assert_eq!(advertisement.capabilities.len(), 1);
    assert_eq!(advertisement.bases.len(), 1);
    assert_eq!(
        advertisement.capabilities[0].capability_id.as_str(),
        "http/request"
    );
    assert_eq!(advertisement.host_id.as_str(), "host/thin");
}

#[test]
fn replacement_fences_stale_provider_and_preserves_enforcement_truth() {
    let mut host = supervisor();
    host.registry_mut()
        .register(base("base/files", 1, "file/copy"))
        .unwrap();
    let old_instance = BaseInstanceId::from("base/files/provider/1");
    host.registry_mut()
        .replace(
            &HostBaseId::from("base/files"),
            &old_instance,
            1,
            base("base/files", 2, "file/copy"),
        )
        .unwrap();

    assert_eq!(
        host.registry_mut().set_lifecycle(
            &HostBaseId::from("base/files"),
            &old_instance,
            1,
            BaseLifecycle::Lost,
        ),
        Err(BaseRegistryRefusal::StaleProvider)
    );
    let current = &host.registry().entries()[0];
    assert_eq!(current.provider_generation, 2);
    assert_eq!(current.enforcement_class, BaseEnforcementClass::Cooperative);
}

#[test]
fn one_provider_cannot_mutate_a_sibling_entry() {
    let mut host = supervisor();
    host.registry_mut()
        .register(base("base/files", 1, "file/copy"))
        .unwrap();
    host.registry_mut()
        .register(base("base/network", 1, "http/request"))
        .unwrap();

    assert_eq!(
        host.registry_mut().set_lifecycle(
            &HostBaseId::from("base/network"),
            &BaseInstanceId::from("base/files/provider/1"),
            1,
            BaseLifecycle::Lost,
        ),
        Err(BaseRegistryRefusal::StaleProvider)
    );
    assert_eq!(host.advertisement().unwrap().capabilities.len(), 2);
}

#[test]
fn projecting_one_ready_base_preserves_existing_owners_and_refuses_duplicates() {
    let mut host = supervisor();
    host.registry_mut()
        .register(base("base/a", 1, "a/work"))
        .unwrap();
    let mut advertisement = host.advertisement().unwrap();
    host.registry_mut()
        .register(base("base/b", 1, "b/work"))
        .unwrap();
    host.registry()
        .project_base_ready_into(&HostBaseId::from("base/b"), &mut advertisement)
        .unwrap();
    assert_eq!(advertisement.bases.len(), 2);
    assert_eq!(advertisement.capabilities.len(), 2);
    let unchanged = advertisement.clone();
    assert_eq!(
        host.registry()
            .project_base_ready_into(&HostBaseId::from("base/b"), &mut advertisement),
        Err(BaseRegistryRefusal::DuplicateBase)
    );
    assert_eq!(advertisement, unchanged);
    host.registry_mut()
        .set_lifecycle(
            &HostBaseId::from("base/b"),
            &BaseInstanceId::from("base/b/provider/1"),
            1,
            BaseLifecycle::Stopped,
        )
        .unwrap();
    assert_eq!(
        host.registry()
            .project_base_ready_into(&HostBaseId::from("base/b"), &mut advertisement),
        Err(BaseRegistryRefusal::BaseNotReady)
    );
    assert_eq!(advertisement, unchanged);
}

#[test]
fn borrowed_entry_checks_match_ordinary_registration_refusals() {
    use conduit_core::{validate_base_provider_entry_ref, BaseProviderEntryRef};
    for case in 0..8 {
        let mut entry = base("base/current", 1, "cap/current");
        match case {
            1 => entry.base_id = "".into(),
            2 => entry.provider_instance_id = "".into(),
            3 => entry.implementation_id = "".into(),
            4 => entry.mechanism_family = "".into(),
            5 => entry.provider_generation = 0,
            6 => entry.capabilities.push(capability("cap/second")),
            7 => {
                entry.base_id = "".into();
                entry.provider_generation = 0;
            }
            _ => {}
        }
        let owned = BaseProviderEntryRef::from(&entry);
        let capabilities: Vec<_> = entry.capabilities.iter().map(Into::into).collect();
        let resources: Vec<_> = entry.resources.iter().map(Into::into).collect();
        let borrowed = BaseProviderEntryRef {
            capabilities: conduit_core::CapabilityOfferRefs::Borrowed(&capabilities),
            resources: conduit_core::ResourceOfferRefs::Borrowed(&resources),
            ..owned
        };
        let expected = BaseRegistry::new(limits()).unwrap().register(entry.clone());
        assert_eq!(
            validate_base_provider_entry_ref(owned, limits()),
            expected,
            "owned case {case}"
        );
        assert_eq!(
            validate_base_provider_entry_ref(borrowed, limits()),
            expected,
            "borrowed case {case}"
        );
    }
}

#[test]
fn prepared_registry_update_drop_is_inert_and_commit_moves_only_selected_provider() {
    let mut registry = BaseRegistry::new(limits()).unwrap();
    registry.register(base("sensor", 1, "sensor/read")).unwrap();
    let pointer = registry.entries()[0].capabilities[0]
        .kind_id
        .as_str()
        .as_ptr();
    let before = registry.clone();
    drop(
        registry
            .prepare_update(None, base("storage", 1, "storage/write"))
            .unwrap(),
    );
    assert_eq!(registry, before);
    registry
        .prepare_update(None, base("storage", 1, "storage/write"))
        .unwrap()
        .commit();
    assert_eq!(registry.entries().len(), 2);
    assert_eq!(registry.entries()[0], before.entries()[0]);
    assert_eq!(
        registry.entries()[0].capabilities[0]
            .kind_id
            .as_str()
            .as_ptr(),
        pointer
    );
    let prior = registry.entries()[1].clone();
    let before = registry.clone();
    drop(
        registry
            .prepare_update(
                Some((&prior.provider_instance_id, prior.provider_generation)),
                base("storage", 2, "storage/read"),
            )
            .unwrap(),
    );
    assert_eq!(registry, before);
    registry
        .prepare_update(
            Some((&prior.provider_instance_id, prior.provider_generation)),
            base("storage", 2, "storage/read"),
        )
        .unwrap()
        .commit();
    assert_eq!(registry.entries()[1].provider_generation, 2);
    assert_eq!(
        registry.entries()[0].capabilities[0]
            .kind_id
            .as_str()
            .as_ptr(),
        pointer
    );
}

#[test]
fn prepared_registry_update_refuses_stale_or_nonadvancing_provider_before_commit() {
    let mut registry = BaseRegistry::new(limits()).unwrap();
    let current = base("storage", 2, "storage/read");
    registry.register(current.clone()).unwrap();
    let before = registry.clone();
    assert!(matches!(
        registry.prepare_update(
            Some((&current.provider_instance_id, 1)),
            base("storage", 3, "storage/write")
        ),
        Err(BaseRegistryRefusal::StaleProvider)
    ));
    assert_eq!(registry, before);
    assert!(matches!(
        registry.prepare_update(
            Some((&current.provider_instance_id, 2)),
            base("storage", 2, "storage/write")
        ),
        Err(BaseRegistryRefusal::GenerationDidNotAdvance)
    ));
    assert_eq!(registry, before);
}
