//! Independent local authority/reference fixtures, not physical admission proof.
use super::*;
use crate::tests::placement;
use conduit_core::{ResourceReferenceAccessRefusal as E, ResourceReferenceAvailability as A};
fn selection(p: &PlannedGear) -> SnapshotRecordSelection<'_> {
    SnapshotRecordSelection {
        host: &p.host_id,
        boot: &p.boot_id,
        kind: &p.kind_id,
        capability: &p.capability_id,
        binding: &p.resources[0],
        host_call: &p.host_calls[0],
        authority: &p.authority[0],
    }
}
fn local(
    p: &PlannedGear,
    reference: &BoundedResourceRef,
) -> (AuthorityGrant, ResourceReferenceBinding) {
    // These are independently selected local fixtures. Production callers may
    // not manufacture them from the incoming selection as this fixture does.
    let authority = &p.authority[0];
    (
        AuthorityGrant {
            grant_id: authority.grant_id.clone(),
            contract_id: authority.contract_id.clone(),
            host_call_contract_id: authority.host_call_contract_id.clone(),
            subject_kind: authority.subject_kind.clone(),
            host_id: authority.host_id.clone(),
            boot_id: authority.boot_id.clone(),
            capability_id: authority.capability_id.clone(),
        },
        ResourceReferenceBinding {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile.clone(),
            access_class: reference.access_class.clone(),
            handle: "local/retained-slot".into(),
            authority_contract: AUTHORITY_CONTRACT.into(),
            authority_grant: authority.grant_id.clone(),
            maximum_bytes: reference.extent.bytes,
            maximum_items: reference.extent.items,
            availability: A::Available,
        },
    )
}
#[test]
fn compact_write_read_preserve_rich_record_key_bytes_and_new_boot_recovery() {
    let (write, reference) = placement("boot/one", true, 2);
    let (grant, local) = local(&write, &reference);
    let mut rich = PreparedSnapshotRecord::prepare(&write, &reference).unwrap();
    let mut compact =
        PreparedSnapshotRecord::prepare_selected(selection(&write), &reference, &grant, &local)
            .unwrap();
    assert_eq!(rich.storage_key(), compact.storage_key());
    let bytes = rich
        .publication(&write.authority[0], b"[]")
        .unwrap()
        .to_vec();
    assert_eq!(
        compact.publication(&write.authority[0], b"[]").unwrap(),
        bytes
    );
    let (read, _) = placement("boot/two", false, 2);
    let (grant, local) = self::local(&read, &reference);
    let restored =
        PreparedSnapshotRecord::prepare_selected(selection(&read), &reference, &grant, &local)
            .unwrap();
    assert_eq!(restored.storage_key(), compact.storage_key());
    assert_eq!(restored.restore(&read.authority[0], &bytes).unwrap(), b"[]");
}
#[test]
fn independent_reference_grant_refuses_stale_lost_identity_and_ceiling_mismatches() {
    let (write, reference) = placement("boot/one", true, 2);
    for change in 0..9 {
        let (grant, mut local) = local(&write, &reference);
        let expected = match change {
            0 => {
                local.version = conduit_core::ResourceVersionIdentity::from_digest([7; 32]);
                E::StaleVersion
            }
            1 => {
                local.maximum_bytes = 1;
                E::ByteBoundExceeded
            }
            2 => {
                local.maximum_items = Some(0);
                E::ItemBoundExceeded
            }
            3 => {
                local.availability = A::Lost;
                E::ResourceLost
            }
            4 => {
                local.availability = A::Stale;
                E::ResourceStale
            }
            5 => {
                local.identity = conduit_core::ResourceSemanticIdentity::from_digest([7; 32]);
                E::SemanticIdentityMismatch
            }
            6 => {
                local.handle = "".into();
                E::EmptyHandle
            }
            7 => {
                local.access_class = "other".into();
                E::AccessClassMismatch
            }
            _ => {
                local.authority_contract = "other".into();
                E::AuthorityContractMismatch
            }
        };
        assert!(
            matches!(PreparedSnapshotRecord::prepare_selected(selection(&write),&reference,&grant,&local),Err(SnapshotRefusal::LocalReference(e)) if e==expected)
        );
    }
}
#[test]
fn original_local_grant_must_match_operation_scope_subject_capability_and_reference_grant() {
    let (write, reference) = placement("boot/one", true, 2);
    for change in 0..8 {
        let (mut grant, mut local) = local(&write, &reference);
        match change {
            0 => grant.host_id = "foreign".into(),
            1 => grant.boot_id = "stale".into(),
            2 => grant.host_call_contract_id = READ_OPERATION.into(),
            3 => grant.subject_kind = "foreign".into(),
            4 => grant.capability_id = "foreign".into(),
            5 => grant.contract_id = "foreign".into(),
            6 => grant.grant_id = "foreign".into(),
            _ => local.authority_grant = "foreign".into(),
        }
        assert!(PreparedSnapshotRecord::prepare_selected(
            selection(&write),
            &reference,
            &grant,
            &local
        )
        .is_err());
    }
}
