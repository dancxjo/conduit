#![cfg(target_os = "linux")]
use conduit_core::*;
use conduit_std_host::scoped_namespace::*;
use std::{
    fs::{self, File},
    os::unix::fs::symlink,
};

fn limits() -> NamespaceLimits {
    NamespaceLimits {
        maximum_entries: 8,
        maximum_name_bytes: 256,
        maximum_read_bytes: 256,
        maximum_capabilities: 4,
        maximum_handles: 11,
        maximum_operations: 8,
    }
}
fn binding() -> BaseProviderBinding {
    BaseProviderBinding {
        base_id: "base/namespace".into(),
        provider_instance_id: "provider/namespace".into(),
        provider_generation: 1,
        implementation_id: "namespace/read-only@1".into(),
        mechanism_family: "base/files".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
    }
}
fn provider(path: &std::path::Path, key: u8) -> ScopedNamespace {
    ScopedNamespace::prepare(
        File::open(path).unwrap(),
        "host".into(),
        "boot".into(),
        binding(),
        "root".into(),
        ResourceGenerationId("root/1".into()),
        limits(),
        [key; 32],
    )
    .unwrap()
}
// This fixture introduces no installed semantic Kind. It exercises the ordinary
// checker/planner selecting a Base-owned Back with existing resource/authority
// classes. A product namespace Fore remains the #6193 owner's responsibility.
fn admission_full(
    operation: &str,
) -> (PlanFragment, CapabilityIssueRequest, ResourceAdmissionOwner) {
    let kind = Kind {
        kind_id: "fixture/namespace".into(),
        kind_contract_revision: "fixture@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![],
        outputs: vec![],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 256,
        },
    };
    let mut catalog = conduit_plot::ProfileCatalog::new();
    catalog.insert_kind(kind.clone()).unwrap();
    let plot =
        conduit_plot::parse("plot probe {\n    access: fixture/namespace\n}\n", &catalog).unwrap();
    let requirement = AuthorityRequirement {
        contract_id: "authority/read".into(),
        host_call_contract_id: operation.into(),
        subject_kind: "files/selected-root".into(),
    };
    let grant = AuthorityGrant {
        grant_id: "grant/read".into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: operation.into(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: "host".into(),
        boot_id: "boot".into(),
        capability_id: "namespace/read".into(),
    };
    let offer = capability_offer_from_parts! {
        semantic_contract: Default::default(), startup_parameters: vec![], shorthand: None,
        capability_id: grant.capability_id.clone(), kind_id:kind.kind_id,
        kind_contract_revision:kind.kind_contract_revision, inputs:vec![], outputs:vec![],
        implementation:ImplementationOffer { implementation_id:"namespace/read-only@1".into(),
            artifact_id:"fixture".into(), execution_profile_id:"cooperative".into() },
        host_calls:vec![HostCallRequirement { contract_id:operation.into(), target_kind:Some(requirement.subject_kind.clone()),
            maximum_in_flight:1, maximum_input_bytes:256, maximum_output_bytes:2048 }],
        resource_requirements:vec![ResourceRequirement { class_id:"resource/root".into(), units:1,
            protected_role:None, compute:None, content:None }], authority_requirements:vec![requirement], limits:kind.limits,
    };
    let mut registry = BaseRegistry::new(BaseRegistryLimits {
        maximum_bases: 1,
        maximum_capabilities_per_base: 1,
        maximum_resources_per_base: 1,
        maximum_advertised_capabilities: 1,
        maximum_advertised_resources: 1,
    })
    .unwrap();
    let selected = binding();
    registry
        .register(BaseProviderEntry {
            base_id: selected.base_id,
            provider_instance_id: selected.provider_instance_id,
            provider_generation: 1,
            implementation_id: selected.implementation_id,
            mechanism_family: selected.mechanism_family,
            enforcement_class: selected.enforcement_class,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![ResourceOffer {
                pool_id: "root".into(),
                class_id: "resource/root".into(),
                capacity_units: 1,
                compute: None,
                content: None,
            }],
        })
        .unwrap();
    let host = ThinHostSupervisor::new("host".into(), "boot".into(), "fixture".into(), registry)
        .unwrap()
        .advertisement()
        .unwrap();
    let mut absent = host.clone();
    absent.capabilities.clear();
    absent.bases.clear();
    assert!(conduit_planner::default_placements(&plot, &[absent]).is_err());
    let placements =
        conduit_planner::default_placements(&plot, std::slice::from_ref(&host)).unwrap();
    let plan = conduit_planner::plan_with_authority_grants(
        &plot,
        std::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        std::slice::from_ref(&grant),
    )
    .unwrap();
    let observations = host
        .resources
        .iter()
        .map(|resource| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: resource.pool_id.clone(),
            class_id: resource.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: resource.capacity_units,
            utilized_units: 0,
            sign_id: "sign/root".into(),
        })
        .collect::<Vec<_>>();
    let mut owner = ResourceAdmissionOwner::new(host);
    for placement in &plan.fragments[0].placements {
        owner
            .admit_planned_placement(plan.plan_id.clone(), placement, &observations)
            .unwrap();
    }
    let active = bind_active_play(&plan.plan_id, &grant.host_id, &grant.boot_id, 1);
    let fragment = plan.fragments[0].clone();
    let gear = &fragment.placements[0];
    assert_eq!(gear.base.as_ref(), Some(&binding()));
    let scope = BaseCapabilityScope {
        host_id: grant.host_id.clone(),
        boot_id: grant.boot_id.clone(),
        base_instance_id: "provider/namespace".into(),
        base_provider_generation: 1,
        plan_id: plan.plan_id,
        active_play_id: active.active_play_id,
        authority_grant_id: grant.grant_id.clone(),
        authority_contract_id: grant.contract_id.clone(),
        capability_id: grant.capability_id.clone(),
        implementation_id: gear.implementation_id.clone(),
        operation_contract_id: operation.into(),
        subject_kind: grant.subject_kind.clone(),
        resource_pool_id: "root".into(),
        resource_generation_id: ResourceGenerationId("root/1".into()),
        envelope_id: "root/read-only".into(),
        maximum_parameter_bytes: 256,
        maximum_result_bytes: 2048,
        maximum_work_units: 256,
        maximum_in_flight: 1,
        maximum_operations: 8,
    };
    let authority = BaseCapabilityAuthority {
        grant,
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: 256,
        maximum_result_bytes: 2048,
        maximum_work_units: 256,
        maximum_in_flight: 1,
        maximum_operations: 8,
    };
    (fragment, CapabilityIssueRequest { scope, authority }, owner)
}
fn admission(operation: &str) -> (PlanFragment, CapabilityIssueRequest) {
    let (fragment, request, _) = admission_full(operation);
    (fragment, request)
}
fn claim(request: &CapabilityIssueRequest) -> BaseOperationClaim {
    let s = &request.scope;
    BaseOperationClaim {
        host_id: s.host_id.clone(),
        boot_id: s.boot_id.clone(),
        base_instance_id: s.base_instance_id.clone(),
        base_provider_generation: s.base_provider_generation,
        plan_id: s.plan_id.clone(),
        active_play_id: s.active_play_id.clone(),
        implementation_id: s.implementation_id.clone(),
        operation_contract_id: s.operation_contract_id.clone(),
        subject_kind: s.subject_kind.clone(),
        resource_pool_id: s.resource_pool_id.clone(),
        resource_generation_id: s.resource_generation_id.clone(),
        envelope_id: s.envelope_id.clone(),
        parameter_bytes: 0,
        work_units: 0,
    }
}
fn issue(
    provider: &mut ScopedNamespace,
    operation: &str,
) -> (NamespaceCapability, BaseOperationClaim) {
    let (fragment, request) = admission(operation);
    let c = claim(&request);
    let handle = provider
        .issue(request, &fragment, &fragment.placements[0].placement_id)
        .unwrap();
    (handle, c)
}
#[test]
fn selected_root_lists_opens_and_survives_root_rename() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("hello"), b"hello").unwrap();
    let mut provider = provider(&root, 1);
    let (mut list, list_claim) = issue(&mut provider, LIST_CALL);
    let id = provider.list(&mut list, &list_claim).unwrap()[0]
        .entry_id
        .clone();
    fs::rename(&root, temp.path().join("moved")).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("hello"), b"intruder").unwrap();
    let (mut read, c) = issue(&mut provider, OPEN_CALL);
    let mut out = [0; 256];
    assert_eq!(provider.open_read(&mut read, &c, &id, &mut out).unwrap(), 5);
    assert_eq!(&out[..5], b"hello");
}
#[test]
fn wrong_scope_forged_possession_and_revocation_have_no_external_effect() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("sentinel"), b"unchanged").unwrap();
    let mut owner = provider(temp.path(), 1);
    let mut attacker = provider(temp.path(), 2);
    let (mut forged, c) = issue(&mut attacker, LIST_CALL);
    assert!(matches!(
        owner.list(&mut forged, &c),
        Err(NamespaceRefusal::Capability(
            BaseCapabilityRefusal::UnknownCapability
        ))
    ));
    let (mut handle, c) = issue(&mut owner, LIST_CALL);
    let mut wrong = c.clone();
    wrong.resource_pool_id = "elsewhere".into();
    assert!(matches!(
        owner.list(&mut handle, &wrong),
        Err(NamespaceRefusal::WrongRoot)
    ));
    for mutate in [0, 1, 2, 3] {
        let mut bad = c.clone();
        match mutate {
            0 => bad.boot_id = "replacement".into(),
            1 => bad.base_provider_generation = 2,
            2 => bad.active_play_id = "other".into(),
            _ => bad.plan_id = "other".into(),
        };
        assert!(owner.list(&mut handle, &bad).is_err());
    }
    let mut wrong = c.clone();
    wrong.operation_contract_id = OPEN_CALL.into();
    assert!(owner.list(&mut handle, &wrong).is_err());
    owner.revoke_play(&c.active_play_id);
    assert!(matches!(
        owner.list(&mut handle, &c),
        Err(NamespaceRefusal::Capability(BaseCapabilityRefusal::Revoked))
    ));
    assert_eq!(
        fs::read(temp.path().join("sentinel")).unwrap(),
        b"unchanged"
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}
#[test]
fn traversal_symlink_replacement_and_oversize_refuse_without_partial_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(temp.path().join("outside"), b"secret").unwrap();
    symlink("../outside", root.join("link")).unwrap();
    fs::write(root.join("file"), b"abcd").unwrap();
    let mut p = provider(&root, 1);
    let (mut list, c) = issue(&mut p, LIST_CALL);
    let entries = p.list(&mut list, &c).unwrap();
    let file = entries
        .iter()
        .find(|e| e.display_name == "file")
        .unwrap()
        .entry_id
        .clone();
    let link = entries
        .iter()
        .find(|e| e.display_name == "link")
        .unwrap()
        .entry_id
        .clone();
    let (mut h, c) = issue(&mut p, OPEN_CALL);
    let mut out = [9; 2];
    assert!(p
        .open_read(&mut h, &c, &ResourceEntryId("../outside".into()), &mut out)
        .is_err());
    assert!(p.open_read(&mut h, &c, &link, &mut out).is_err());
    assert_eq!(out, [0; 2]);
    assert!(matches!(
        p.open_read(&mut h, &c, &file, &mut out),
        Err(NamespaceRefusal::ByteBound)
    ));
    assert_eq!(out, [0; 2]);
    fs::rename(root.join("file"), root.join("old")).unwrap();
    fs::write(root.join("file"), b"new").unwrap();
    assert!(matches!(
        p.open_read(&mut h, &c, &file, &mut out),
        Err(NamespaceRefusal::ChangedEntry)
    ));
    assert_eq!(fs::read(temp.path().join("outside")).unwrap(), b"secret");
}
#[test]
fn finite_membership_capability_and_operation_slots_refuse() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("a"), b"a").unwrap();
    fs::write(temp.path().join("b"), b"b").unwrap();
    let mut small = limits();
    small.maximum_entries = 1;
    assert!(matches!(
        ScopedNamespace::prepare(
            File::open(temp.path()).unwrap(),
            "host".into(),
            "boot".into(),
            binding(),
            "root".into(),
            ResourceGenerationId("root/1".into()),
            small,
            [1; 32]
        ),
        Err(NamespaceRefusal::EntryBound)
    ));
    let mut p = provider(temp.path(), 1);
    let (fragment, mut request) = admission(LIST_CALL);
    request.scope.maximum_operations = 1;
    let c = claim(&request);
    let mut handle = p
        .issue(
            request.clone(),
            &fragment,
            &fragment.placements[0].placement_id,
        )
        .unwrap();
    p.list(&mut handle, &c).unwrap();
    assert!(matches!(
        p.list(&mut handle, &c),
        Err(NamespaceRefusal::Capability(
            BaseCapabilityRefusal::Exhausted
        ))
    ));
    for _ in 0..3 {
        p.issue(
            request.clone(),
            &fragment,
            &fragment.placements[0].placement_id,
        )
        .unwrap();
    }
    assert!(matches!(
        p.issue(request, &fragment, &fragment.placements[0].placement_id),
        Err(NamespaceRefusal::Capability(
            BaseCapabilityRefusal::TableFull
        ))
    ));
}

#[path = "scoped_namespace/play.rs"]
mod play;

#[test]
fn replaced_boot_provider_root_and_invalid_admission_do_not_inherit_possession() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("input"), b"input").unwrap();
    let mut original = provider(temp.path(), 1);
    let (mut old, c) = issue(&mut original, LIST_CALL);
    for replacement in [0, 1, 2] {
        let mut selected = binding();
        let mut boot = BootId::from("boot");
        let mut generation = ResourceGenerationId("root/1".into());
        match replacement {
            0 => boot = "boot/replacement".into(),
            1 => selected.provider_generation = 2,
            _ => generation = ResourceGenerationId("root/2".into()),
        }
        let mut fresh = ScopedNamespace::prepare(
            File::open(temp.path()).unwrap(),
            "host".into(),
            boot,
            selected,
            "root".into(),
            generation,
            limits(),
            [2; 32],
        )
        .unwrap();
        assert!(fresh.list(&mut old, &c).is_err());
    }
    let (fragment, request) = admission(OPEN_CALL);
    for mutation in [0, 1, 2, 3] {
        let mut invalid = request.clone();
        match mutation {
            0 => invalid.scope.resource_pool_id = "other".into(),
            1 => invalid.scope.maximum_result_bytes = 2049,
            2 => invalid.authority.grant.grant_id = "forged".into(),
            _ => invalid.scope.base_provider_generation = 2,
        };
        assert!(original
            .issue(invalid, &fragment, &fragment.placements[0].placement_id)
            .is_err());
    }
    let mut unsupported = c.clone();
    unsupported.operation_contract_id = "conduit.host/create-file@1".into();
    assert!(matches!(
        original.list(&mut old, &unsupported),
        Err(NamespaceRefusal::UnsupportedOperation)
    ));
    assert_eq!(fs::read(temp.path().join("input")).unwrap(), b"input");
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn provider_loss_is_terminal_and_precedes_effects() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("sentinel"), b"unchanged").unwrap();
    let mut owner = provider(temp.path(), 1);
    let (mut handle, c) = issue(&mut owner, LIST_CALL);
    owner.retire();
    assert!(matches!(
        owner.list(&mut handle, &c),
        Err(NamespaceRefusal::Unavailable)
    ));
    let (fragment, request) = admission(LIST_CALL);
    assert!(matches!(
        owner.issue(request, &fragment, &fragment.placements[0].placement_id),
        Err(NamespaceRefusal::Unavailable)
    ));
    assert_eq!(
        fs::read(temp.path().join("sentinel")).unwrap(),
        b"unchanged"
    );
}
#[path = "scoped_namespace/allocation.rs"]
mod allocation;
