use conduit_core::{
    BaseEnforcementClass, BaseImplementationId, BaseInstanceId, BaseLifecycle, BaseProviderEntry,
    BaseRegistry, BaseRegistryLimits, BaseRegistryRefusal, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, HostBaseId, HostBaseKindId, HostId, HostProfileId, ImplementationOffer,
    KindContractRevision, KindId, ThinHostSupervisor,
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
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(id),
        kind_id: KindId::from(id),
        kind_contract_revision: KindContractRevision::from("revision"),
        inputs: vec![],
        outputs: vec![],
        implementation: ImplementationOffer {
            implementation_id: conduit_core::ImplementationId::from(format!("impl/{id}")),
            artifact_id: conduit_core::ArtifactId::from(format!("artifact/{id}")),
            execution_profile_id: conduit_core::ExecutionProfileId::from("pure-or-base"),
        },
        host_operations: vec![],
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
