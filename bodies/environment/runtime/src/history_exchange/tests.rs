use super::contract::{HISTORY_CALL, HISTORY_MAXIMUM_BYTES};
use super::owner::*;
use super::*;
use conduit_core::*;
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/history/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/history"),
        authority_contract_id: AuthorityContractId::from("authority/history@1"),
        capability_id: CapabilityId::from("machine/history/read-write"),
        implementation_id: ImplementationId::from("implementation/history/fixture@1"),
        operation_contract_id: HostCallContractId::from(HISTORY_CALL),
        subject_kind: KindId::from(super::contract::HISTORY_KIND),
        resource_pool_id: ResourcePoolId::from("history/controller"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("history/window"),
        maximum_parameter_bytes: HISTORY_MAXIMUM_BYTES,
        maximum_result_bytes: HISTORY_MAXIMUM_BYTES,
        maximum_work_units: 8,
        maximum_in_flight: 1,
        maximum_operations: 100000,
    }
}

pub(super) fn selected() -> (
    PlanFragment,
    LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    super::test_support::selected(
        &scope(),
        HistoryReadContract::prepare().unwrap().kind(),
        HISTORY_CALL,
        HISTORY_MAXIMUM_BYTES,
        "history",
        "history",
    )
}

pub(super) fn bind(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    active: &ActivePlayIdentity,
    placement: &PlacementId,
) -> Result<HistoryHostCall<Provider>, HistoryCallRefusal> {
    let mut scope = scope();
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: scope.authority_grant_id.clone(),
            contract_id: scope.authority_contract_id.clone(),
            host_call_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            capability_id: scope.capability_id.clone(),
        },
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: scope.maximum_parameter_bytes,
        maximum_result_bytes: scope.maximum_result_bytes,
        maximum_work_units: scope.maximum_work_units,
        maximum_in_flight: 1,
        maximum_operations: scope.maximum_operations,
    };
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: HISTORY_MAXIMUM_BYTES,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        4,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    unsafe {
        HistoryHostCall::bind_selected(
            table,
            handle,
            claim,
            HistoryAttachment {
                generation: 7,
                resource_bytes: 4096,
            },
            Provider,
            HistoryCallSelection {
                contract: &HistoryReadContract::prepare().unwrap(),
                fragment,
                lowered,
                active,
                placement,
            },
        )
    }
}

#[derive(Default)]
pub(super) struct Provider;
impl HistoryProvider for Provider {
    fn read_signed_batch(
        &mut self,
        cursor: u32,
        out: &mut [u8; MAXIMUM_SIGNED_BATCH_BYTES],
    ) -> Result<usize, HistoryProviderRefusal> {
        out[..4].copy_from_slice(&cursor.to_le_bytes());
        Ok(4)
    }
    fn revoke(&mut self) {}
}
#[test]
fn selected_read_consumes_exact_request_and_rejects_replay() {
    let (fragment, lowered, active, placement) = selected();
    let mut owner = bind(&fragment, &lowered, &active, &placement).unwrap();
    let c = HistoryReadContract::prepare().unwrap();
    let bytes = codec::encode_request(&c, 9).unwrap();
    let result = owner
        .invoke(
            conduit_kernel::NodeId(0),
            conduit_kernel::HostCallId(0),
            conduit_kernel::RequestId(0),
            &bytes,
        )
        .unwrap();
    assert_eq!(
        codec::decode_result(&c, result).unwrap(),
        9u32.to_le_bytes()
    );
    assert_eq!(
        owner.invoke(
            conduit_kernel::NodeId(0),
            conduit_kernel::HostCallId(0),
            conduit_kernel::RequestId(0),
            &bytes
        ),
        Err(HistoryCallRefusal::StaleRequest)
    );
}
#[test]
fn changed_play_refuses_before_read() {
    let (fragment, lowered, mut active, placement) = selected();
    active.active_play_id = conduit_core::ActivePlayId::from("wrong/play");
    assert!(bind(&fragment, &lowered, &active, &placement).is_err());
}
