//! Projection from actual fixture issuers/tables, not native Root approval,
//! entropy admission, storage possession issuance or physical execution proof.
use super::*;
use crate::storage_base::{contract::*, transaction_contract::*};
use alloc::format;

fn authority(pair: usize, role: StorageTransactionRole) -> BaseCapabilityAuthority {
    let (call, kind) = match role {
        StorageTransactionRole::Authorize => (AUTHORIZE_CALL, AUTHORIZE_KIND),
        StorageTransactionRole::Settle => (SETTLE_CALL, SETTLE_KIND),
    };
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: format!("fixture/storage/{pair}/{kind}/grant").into(),
            contract_id: BLOCK_READ_AUTHORITY.into(),
            host_call_contract_id: call.into(),
            subject_kind: kind.into(),
            host_id: "fixture/storage-host".into(),
            boot_id: "fixture/storage-boot".into(),
            capability_id: if pair == 0 {
                role.implementation().into()
            } else {
                format!("{}/reentry", role.implementation()).into()
            },
        },
        base_instance_id: format!("fixture/storage/{pair}/provider").into(),
        base_provider_generation: 1,
        resource_pool_id: format!("fixture/storage/{pair}/pool").into(),
        resource_generation_id: ResourceGenerationId(format!("fixture/storage/{pair}/generation")),
        operation_contract_id: call.into(),
        envelope_id: format!("fixture/storage/{pair}/envelope").into(),
        maximum_parameter_bytes: BLOCK_READ_MAXIMUM_BYTES,
        maximum_result_bytes: BLOCK_READ_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: if pair == 0 { 16 } else { 1 },
    }
}
fn preparation(pair: usize) -> NativeStoragePreparation {
    let a = authority(pair, StorageTransactionRole::Authorize);
    let s = authority(pair, StorageTransactionRole::Settle);
    NativeStoragePreparation {
        host: a.grant.host_id.clone(),
        boot: a.grant.boot_id.clone(),
        provider: a.base_instance_id.clone(),
        pool: a.resource_pool_id.clone(),
        artifact: format!("fixture/storage/{pair}/artifact").into(),
        maximum_retained_bytes: 131072,
        authorize: NativeProtocolIssuer::from_test_authority(a, [40 + pair as u8; 32]).unwrap(),
        settle: NativeProtocolIssuer::from_test_authority(s, [50 + pair as u8; 32]).unwrap(),
        additional: None,
    }
}
fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/storage-host".into(),
        boot_id: "fixture/storage-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn exact_projection(preparation: &NativeStoragePreparation, projected: &HostAdvertisement) {
    let base = projected
        .bases
        .iter()
        .find(|base| base.provider_instance_id == preparation.provider)
        .unwrap();
    assert_eq!(base.provider_generation, 1);
    assert_eq!(base.implementation_id.as_str(), TRANSACTION_BASE);
    assert_eq!(base.resource_pool_ids, vec![preparation.pool.clone()]);
    let contract = BlockReadTransactionContract::prepare().unwrap();
    for (issuer, role, kind) in [
        (
            &preparation.authorize,
            StorageTransactionRole::Authorize,
            contract.authorize_kind(),
        ),
        (
            &preparation.settle,
            StorageTransactionRole::Settle,
            contract.settle_kind(),
        ),
    ] {
        let grant = issuer.grant();
        let offer = projected
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == grant.capability_id)
            .unwrap();
        assert!(base.capability_ids.contains(&grant.capability_id));
        assert_eq!(
            offer.implementation.implementation_id.as_str(),
            role.implementation()
        );
        assert_eq!(offer.kind_id, kind.kind_id);
        assert_eq!(offer.semantic_contract, kind.semantic_contract());
        assert_eq!(offer.limits, kind.limits);
        assert_eq!(offer.limits.max_active_instances, 1);
        assert_eq!(offer.host_calls.len(), 1);
        assert_eq!(offer.host_calls[0].contract_id, grant.host_call_contract_id);
        assert_eq!(offer.authority_requirements.len(), 1);
        assert_eq!(
            offer.authority_requirements[0].contract_id,
            grant.contract_id
        );
        assert_eq!(
            offer.authority_requirements[0].subject_kind,
            grant.subject_kind
        );
        assert_eq!(
            offer.authority_requirements[0].host_call_contract_id,
            grant.host_call_contract_id
        );
    }
    let pool = projected
        .resources
        .iter()
        .find(|pool| pool.pool_id == preparation.pool)
        .unwrap();
    assert_eq!(pool.class_id.as_str(), TRANSACTION_RESOURCE);
    assert_eq!(pool.capacity_units, 2);
}
#[test]
fn original_pair_projects_exact_two_existing_role_offers() {
    let first = preparation(0);
    let mut projected = advertisement();
    first.append(&mut projected).unwrap();
    assert_eq!(projected.capabilities.len(), 2);
    assert_eq!(projected.bases.len(), 1);
    assert_eq!(projected.resources.len(), 1);
    assert_eq!(
        first.authorize.grant().capability_id.as_str(),
        AUTHORIZE_IMPLEMENTATION
    );
    assert_eq!(
        first.settle.grant().capability_id.as_str(),
        SETTLE_IMPLEMENTATION
    );
    exact_projection(&first, &projected);
}
#[test]
fn separately_retained_second_pair_projects_four_distinct_scopes() {
    let mut first = preparation(0);
    first.additional = Some(Box::new(preparation(1)));
    let mut projected = advertisement();
    first.append(&mut projected).unwrap();
    assert_eq!(projected.capabilities.len(), 4);
    assert_eq!(projected.bases.len(), 2);
    assert_eq!(projected.resources.len(), 2);
    for (index, offer) in projected.capabilities.iter().enumerate() {
        assert!(
            projected.capabilities[..index]
                .iter()
                .all(|other| other.capability_id != offer.capability_id)
        );
    }
    assert_ne!(
        projected.bases[0].provider_instance_id,
        projected.bases[1].provider_instance_id
    );
    assert_ne!(
        projected.resources[0].pool_id,
        projected.resources[1].pool_id
    );
    exact_projection(&first, &projected);
    exact_projection(first.additional.as_ref().unwrap(), &projected);
}
#[test]
fn duplicate_provider_pool_and_capability_refuse_projection() {
    for collision in 0..3 {
        let mut first = preparation(0);
        let mut second = preparation(1);
        match collision {
            0 => second.provider = first.provider.clone(),
            1 => second.pool = first.pool.clone(),
            _ => {
                second.authorize = NativeProtocolIssuer::from_test_authority(
                    authority(0, StorageTransactionRole::Authorize),
                    [60; 32],
                )
                .unwrap()
            }
        }
        first.additional = Some(Box::new(second));
        assert!(first.append(&mut advertisement()).is_err());
    }
}
#[test]
fn nested_additional_is_refused_before_any_projection() {
    let mut first = preparation(0);
    let mut second = preparation(1);
    second.additional = Some(Box::new(preparation(2)));
    first.additional = Some(Box::new(second));
    let mut projected = advertisement();
    assert!(first.append(&mut projected).is_err());
    assert!(projected.bases.is_empty());
    assert!(projected.capabilities.is_empty());
    assert!(projected.resources.is_empty());
}
#[test]
fn wrong_host_or_boot_and_repeat_append_refuse() {
    let first = preparation(0);
    for host in [true, false] {
        let mut projected = advertisement();
        if host {
            projected.host_id = "foreign/host".into();
        } else {
            projected.boot_id = "foreign/boot".into();
        }
        assert!(first.append(&mut projected).is_err());
        assert!(projected.capabilities.is_empty());
    }
    let mut projected = advertisement();
    first.append(&mut projected).unwrap();
    assert!(first.append(&mut projected).is_err());
    assert_eq!(projected.capabilities.len(), 2);
}
