//! Genuine four selected storage scopes; semantic owner/preparation tests only.
//! No Body pin is fabricated, no USB effect is submitted, and synthetic semantic
//! settlement receipts here do not prove native/Kernel subordinate retirement.
use super::*;
use crate::machine_membrane::selected_operation::SelectedOperationPlan;
use crate::storage_base::result::{BlockReadDisposition, PreparedBlockReadResultEncoder};
use crate::storage_base::{contract::*, transaction_contract::*, transaction_owner::*};
use alloc::{format, vec};
use conduit_kernel::{HostCallId, RequestId};
use conduit_plan_lowering::lowering::lower_plan_fragment;

fn scope(kind: &str, call: &str, pair: usize) -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: "fixture/storage-host".into(),
        boot_id: "fixture/storage-boot".into(),
        base_instance_id: format!("fixture/storage-provider/{pair}").into(),
        base_provider_generation: 7,
        plan_id: "fixture/unsealed".into(),
        active_play_id: "fixture/unsealed-play".into(),
        authority_grant_id: format!("fixture/grant/{pair}/{kind}").into(),
        authority_contract_id: "authority/storage-block-read@1".into(),
        capability_id: format!("fixture/capability/{pair}/{kind}").into(),
        implementation_id: format!("fixture/implementation/{kind}").into(),
        operation_contract_id: call.into(),
        subject_kind: kind.into(),
        resource_pool_id: format!("fixture/resource/{pair}/{kind}").into(),
        resource_generation_id: ResourceGenerationId("fixture/resource-generation/3".into()),
        envelope_id: format!("fixture/envelope/{pair}/{kind}").into(),
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

fn fixture(
    edit: impl FnOnce(&mut StorageScopeAdmission<'_>),
) -> Result<(Plan, [BlockReadTransactionOwner; 2]), StorageTransactionRefusal> {
    let contract = BlockReadTransactionContract::prepare().unwrap();
    let mut scopes = [
        scope(AUTHORIZE_KIND, AUTHORIZE_CALL, 0),
        scope(SETTLE_KIND, SETTLE_CALL, 0),
        scope(AUTHORIZE_KIND, AUTHORIZE_CALL, 1),
        scope(SETTLE_KIND, SETTLE_CALL, 1),
    ];
    let kinds = [
        contract.authorize_kind(),
        contract.settle_kind(),
        contract.authorize_kind(),
        contract.settle_kind(),
    ];
    let calls = [AUTHORIZE_CALL, SETTLE_CALL, AUTHORIZE_CALL, SETTLE_CALL];
    let mut parts = Vec::new();
    let mut placements = Vec::new();
    for index in 0..4 {
        let (part, _, _, placement) = crate::machine_membrane::selection_fixture::selected(
            &scopes[index],
            kinds[index],
            calls[index],
            4096,
            "storage",
            &format!("storage-pair-{index}"),
        );
        parts.push(part);
        placements.push(placement);
    }
    let mut fragment = parts.remove(0);
    for part in parts {
        fragment.placements.extend(part.placements);
        fragment.startup_order.extend(part.startup_order);
        fragment.expected_sign.extend(part.expected_sign);
    }
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    );
    let fragment = &plan.fragments[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = lower_plan_fragment(fragment).unwrap();
    for scope in &mut scopes {
        scope.plan_id = plan.plan_id.clone();
        scope.active_play_id = active.active_play_id.clone();
    }
    let mut admissions = Vec::new();
    for (index, scope) in scopes.into_iter().enumerate() {
        admissions.push(admission(
            scope,
            SelectedOperationPlan {
                fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placements[index],
            },
            20 + index as u8,
        ));
    }
    edit(&mut admissions[2]);
    let first =
        BlockReadTransactionOwner::bind(&contract, admissions.remove(0), admissions.remove(0))?;
    let second =
        BlockReadTransactionOwner::bind(&contract, admissions.remove(0), admissions.remove(0))?;
    Ok((plan, [first, second]))
}
fn input(lba: u64) -> Vec<u8> {
    let contract = BlockReadContract::prepare().unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!("request record")
    };
    let values = fields
        .iter()
        .zip([512u64, 1, lba])
        .map(|(field, value)| {
            StructuredFieldValue::new(
                field.name(),
                StructuredInfoValue::leaf(field.value_type().clone(), value.to_le_bytes().to_vec())
                    .unwrap(),
            )
            .unwrap()
        })
        .collect();
    StructuredInfoValue::record(contract.request_type().clone(), values)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn bank(plan: &Plan, owners: [BlockReadTransactionOwner; 2]) -> StorageAssociations {
    let [first, second] = owners;
    // These are deliberately uninstalled USB node labels: no Body/native proof
    // is claimed. The bank must deny every USB effect until a genuine pin binds.
    let nodes = [NodeId(65530), NodeId(65531), NodeId(65532)];
    StorageAssociations::prepare(
        StorageAssociation::prepare(first, plan, nodes, 131072, 0).unwrap(),
        Some(StorageAssociation::prepare(second, plan, nodes, 131072, 0).unwrap()),
    )
    .unwrap()
}
#[test]
fn two_independent_scopes_bind_but_unpinned_bank_cannot_enable_usb() {
    let (plan, owners) = fixture(|_| {}).unwrap();
    let mut bank = bank(&plan, owners);
    assert_eq!(bank.pairs().count(), 2);
    let first = bank.pairs_mut().next().unwrap();
    first
        .authorize(
            first.owner.authorize_node(),
            HostCallId(0),
            RequestId(0),
            &input(0),
        )
        .unwrap();
    assert_eq!(bank.active_count(), 1);
    assert!(!bank.allows_usb(NodeId(65530)));
    assert!(matches!(
        bank.pairs_mut().next().unwrap().authorize(
            bank_node(&plan, 0),
            HostCallId(0),
            RequestId(1),
            &input(0)
        ),
        Err(ProtocolCallRefusal::Storage(
            StorageTransactionRefusal::Pressure
        ))
    ));
}
fn bank_node(plan: &Plan, index: usize) -> NodeId {
    lower_plan_fragment(&plan.fragments[0])
        .unwrap()
        .identity
        .node_for_placement(&plan.fragments[0].placements[index].placement_id)
        .unwrap()
}
#[test]
fn second_pair_scope_substitution_is_refused_before_authorization() {
    for field in 0..5 {
        assert!(
            fixture(|admission| match field {
                0 => admission.claim.boot_id = "foreign/boot".into(),
                1 => admission.claim.active_play_id = "foreign/play".into(),
                2 => admission.claim.base_provider_generation += 1,
                3 =>
                    admission.claim.resource_generation_id =
                        ResourceGenerationId("foreign/resource".into()),
                _ => admission.claim.envelope_id = "foreign/envelope".into(),
            })
            .is_err()
        );
    }
}
#[test]
fn first_stalled_owner_stays_terminal_and_independent_second_accepts_fresh_read() {
    let (_, [mut first, mut second]) = fixture(|_| {}).unwrap();
    let (submission, _) = first
        .authorize(
            first.authorize_node(),
            HostCallId(0),
            RequestId(0),
            &input(4),
        )
        .unwrap();
    let contract = BlockReadContract::prepare().unwrap();
    let mut encoder = PreparedBlockReadResultEncoder::new(&contract).unwrap();
    let bytes = encoder
        .disposition(
            submission.request(),
            u64::from(submission.ordinal()),
            BlockReadDisposition::Stalled,
        )
        .unwrap();
    // SAFETY: semantic-owner fixture submits no physical transfer or subordinate
    // Kernel call. This synthetic boundary tests scope rotation, not native ack.
    let receipt = unsafe { StorageSettlementReceipt::acknowledged(submission) };
    first
        .settle(
            first.settle_node(),
            HostCallId(0),
            RequestId(0),
            bytes,
            receipt,
        )
        .unwrap_or_else(|(error, _retained_receipt)| panic!("settlement refused: {error:?}"));
    assert!(matches!(
        first.authorize(
            first.authorize_node(),
            HostCallId(0),
            RequestId(1),
            &input(5)
        ),
        Err(StorageTransactionRefusal::Terminal)
    ));
    let (fresh, _) = second
        .authorize(
            second.authorize_node(),
            HostCallId(0),
            RequestId(0),
            &input(5),
        )
        .unwrap();
    assert_eq!(fresh.request().lba, 5);
    assert_eq!(fresh.ordinal(), 0);
}
#[test]
fn wrong_settle_node_preserves_exact_other_owner_submission() {
    let (_, [mut first, second]) = fixture(|_| {}).unwrap();
    let (submission, _) = first
        .authorize(
            first.authorize_node(),
            HostCallId(0),
            RequestId(0),
            &input(7),
        )
        .unwrap();
    let mut encoder =
        PreparedBlockReadResultEncoder::new(&BlockReadContract::prepare().unwrap()).unwrap();
    let bytes = encoder
        .disposition(
            submission.request(),
            u64::from(submission.ordinal()),
            BlockReadDisposition::Stalled,
        )
        .unwrap();
    // SAFETY: no physical DMA or Kernel call exists in this semantic fixture.
    let receipt = unsafe { StorageSettlementReceipt::acknowledged(submission) };
    let (error, receipt) = first
        .settle(
            second.settle_node(),
            HostCallId(0),
            RequestId(0),
            bytes,
            receipt,
        )
        .unwrap_err();
    assert_eq!(error, StorageTransactionRefusal::Binding);
    assert!(matches!(
        first.authorize(
            first.authorize_node(),
            HostCallId(0),
            RequestId(1),
            &input(8)
        ),
        Err(StorageTransactionRefusal::Pressure)
    ));
    first
        .settle(
            first.settle_node(),
            HostCallId(0),
            RequestId(0),
            bytes,
            receipt,
        )
        .unwrap_or_else(|(error, _retained_receipt)| panic!("settlement refused: {error:?}"));
}
#[test]
fn bank_retention_and_duplicate_selected_nodes_are_refused() {
    let (plan, [first, _]) = fixture(|_| {}).unwrap();
    assert!(
        StorageAssociation::prepare(
            first,
            &plan,
            [NodeId(65530), NodeId(65531), NodeId(65532)],
            1,
            0
        )
        .is_err()
    );
    let (plan, [first, _]) = fixture(|_| {}).unwrap();
    let (_, [duplicate, _]) = fixture(|_| {}).unwrap();
    let nodes = [NodeId(65530), NodeId(65531), NodeId(65532)];
    let first = StorageAssociation::prepare(first, &plan, nodes, 131072, 0).unwrap();
    let duplicate = StorageAssociation::prepare(duplicate, &plan, nodes, 131072, 0).unwrap();
    assert!(StorageAssociations::prepare(first, Some(duplicate)).is_err());
}
