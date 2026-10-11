//! Cooperative admission/lifetime fixtures; no native DMA or controller proof.
use super::*;
use alloc::vec;
use conduit_core::*;
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/serial/duplex-bytes/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/visible"),
        authority_contract_id: AuthorityContractId::from("authority/serial-duplex-bytes@1"),
        capability_id: CapabilityId::from("serial/duplex-bytes/receive"),
        implementation_id: ImplementationId::from("implementation/serial-duplex-bytes/fixture@1"),
        operation_contract_id: HostCallContractId::from("conduit.host/serial-duplex-bytes@1"),
        subject_kind: KindId::from("serial/duplex-bytes"),
        resource_pool_id: ResourcePoolId::from("serial/duplex-bytes/controller"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("serial/duplex-bytes/controller/window-8"),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 8,
        maximum_in_flight: 1,
        maximum_operations: 100000,
    }
}

fn authority() -> BaseCapabilityAuthority {
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: AuthorityGrantId::from("grant/visible"),
            contract_id: AuthorityContractId::from("authority/serial-duplex-bytes@1"),
            host_call_contract_id: HostCallContractId::from("conduit.host/serial-duplex-bytes@1"),
            subject_kind: KindId::from("serial/duplex-bytes"),
            host_id: HostId::from("host/one"),
            boot_id: BootId::from("boot/one"),
            capability_id: CapabilityId::from("serial/duplex-bytes/receive"),
        },
        base_instance_id: BaseInstanceId::from("base/serial/duplex-bytes/instance"),
        base_provider_generation: 4,
        resource_pool_id: ResourcePoolId::from("serial/duplex-bytes/controller"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        operation_contract_id: HostCallContractId::from("conduit.host/serial-duplex-bytes@1"),
        envelope_id: CapabilityEnvelopeId::from("serial/duplex-bytes/controller/window-8"),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 32,
        maximum_in_flight: 1,
        maximum_operations: 100000,
    }
}

pub(super) fn request() -> CapabilityIssueRequest {
    CapabilityIssueRequest {
        scope: scope(),
        authority: authority(),
    }
}

pub(super) fn table(key: u8) -> BaseCapabilityTable {
    BaseCapabilityTable::new(
        HostId::from("host/one"),
        BootId::from("boot/one"),
        BaseInstanceId::from("base/serial/duplex-bytes/instance"),
        4,
        [key; 32],
        4,
    )
    .unwrap()
}

pub(super) fn claim() -> BaseOperationClaim {
    let scope = scope();
    BaseOperationClaim {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_instance_id: scope.base_instance_id,
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id,
        active_play_id: scope.active_play_id,
        implementation_id: scope.implementation_id,
        operation_contract_id: scope.operation_contract_id,
        subject_kind: scope.subject_kind,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        parameter_bytes: 4096,
        work_units: 1,
    }
}

pub(super) fn selected() -> (
    PlanFragment,
    LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    crate::machine_membrane::selection_fixture::selected(
        &scope(),
        SerialDuplexContract::prepare().unwrap().kind(),
        SERIAL_DUPLEX_CALL,
        SERIAL_DUPLEX_MAXIMUM_BYTES,
        "serial/duplex-bytes",
        "serial-duplex-bytes",
    )
}

pub(super) fn owner(key: u8) -> SerialDuplexCallOwner {
    let (fragment, lowered, active, placement) = selected();
    bind(
        SelectedOperationPlan {
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement_id: &placement,
        },
        key,
    )
    .unwrap()
}

pub(super) fn bind(
    selection: SelectedOperationPlan<'_>,
    key: u8,
) -> Result<SerialDuplexCallOwner, SerialOwnerRefusal> {
    let SelectedOperationPlan {
        fragment,
        lowered,
        active,
        placement_id: placement,
    } = selection;
    let mut table = table(key);
    let mut request = request();
    request.scope.plan_id = fragment.plan_id.clone();
    request.scope.active_play_id = active.active_play_id.clone();
    let handle = table.issue(request).unwrap();
    let mut claim = claim();
    claim.plan_id = fragment.plan_id.clone();
    claim.active_play_id = active.active_play_id.clone();
    unsafe {
        SerialDuplexCallOwner::bind_admitted(
            table,
            handle,
            claim,
            &SerialDuplexContract::prepare().unwrap(),
            SelectedOperationPlan {
                fragment,
                lowered,
                active,
                placement_id: placement,
            },
        )
    }
}

pub(super) fn input(tag: &str, bytes: &[u8]) -> alloc::vec::Vec<u8> {
    let contract = SerialDuplexContract::prepare().unwrap();
    let StructuredInfoTypeShape::Variant { cases, .. } = contract.request_type().shape() else {
        panic!();
    };
    let payload_type = cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type();
    let StructuredInfoTypeShape::Record { fields, .. } = payload_type.shape() else {
        panic!();
    };
    let field = &fields[0];
    let payload = StructuredInfoValue::record(
        payload_type.clone(),
        vec![StructuredFieldValue::new(
            field.name(),
            StructuredInfoValue::leaf(field.value_type().clone(), bytes.to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    StructuredInfoValue::variant(contract.request_type().clone(), tag, payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
