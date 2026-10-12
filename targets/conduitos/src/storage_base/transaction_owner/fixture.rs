//! Actual selected placements and separate cooperative storage possessions.
use super::super::*;
use alloc::{format, vec};
use conduit_plan_lowering::lowering::lower_plan_fragment;
fn scope(kind: &str, call: &str) -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: "fixture/storage-host".into(),
        boot_id: "fixture/storage-boot".into(),
        base_instance_id: "fixture/storage-provider".into(),
        base_provider_generation: 7,
        plan_id: "fixture/unsealed".into(),
        active_play_id: "fixture/unsealed-play".into(),
        authority_grant_id: format!("fixture/grant/{kind}").into(),
        authority_contract_id: "authority/storage-block-read@1".into(),
        capability_id: kind.into(),
        implementation_id: format!("fixture/implementation/{kind}").into(),
        operation_contract_id: call.into(),
        subject_kind: kind.into(),
        resource_pool_id: format!("fixture/resource/{kind}").into(),
        resource_generation_id: ResourceGenerationId("fixture/resource-generation/3".into()),
        envelope_id: format!("fixture/envelope/{kind}").into(),
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 16,
    }
}
fn admission<'a>(
    scope: BaseCapabilityScope,
    selected: SelectedOperationPlan<'a>,
    key: u8,
) -> StorageScopeAdmission<'a> {
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
        maximum_parameter_bytes: 4096,
        maximum_result_bytes: 4096,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 16,
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
        parameter_bytes: 4096,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [key; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    StorageScopeAdmission {
        table,
        handle,
        claim,
        selected,
    }
}
pub(super) fn owner(
    edit: impl FnOnce(&mut StorageScopeAdmission<'_>, &mut StorageScopeAdmission<'_>),
) -> Result<BlockReadTransactionOwner, StorageTransactionRefusal> {
    let contract = BlockReadTransactionContract::prepare().unwrap();
    let mut a = scope(AUTHORIZE_KIND, AUTHORIZE_CALL);
    let mut s = scope(SETTLE_KIND, SETTLE_CALL);
    let (mut fragment, _, _, a_placement) = crate::machine_membrane::selection_fixture::selected(
        &a,
        contract.authorize_kind(),
        AUTHORIZE_CALL,
        4096,
        "storage",
        "storage-authorize",
    );
    let (second, _, _, s_placement) = crate::machine_membrane::selection_fixture::selected(
        &s,
        contract.settle_kind(),
        SETTLE_CALL,
        4096,
        "storage",
        "storage-settle",
    );
    fragment.placements.extend(second.placements);
    fragment.startup_order.push(s_placement.clone());
    fragment.expected_sign.extend([
        ExpectedSign::PlacementPrepared(s_placement.clone()),
        ExpectedSign::PlacementTerminal(s_placement.clone()),
    ]);
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    for scope in [&mut a, &mut s] {
        scope.plan_id = fragment.plan_id.clone();
        scope.active_play_id = active.active_play_id.clone();
    }
    let selected = |placement_id| SelectedOperationPlan {
        fragment: &fragment,
        lowered: &lowered,
        active: &active,
        placement_id,
    };
    let mut a = admission(a, selected(&a_placement), 4);
    let mut s = admission(s, selected(&s_placement), 5);
    edit(&mut a, &mut s);
    BlockReadTransactionOwner::bind(&contract, a, s)
}
pub(super) fn input(lba: u64) -> alloc::vec::Vec<u8> {
    crate::storage_base::tests::request_bytes(&BlockReadContract::prepare().unwrap(), 512, 1, lba)
}
