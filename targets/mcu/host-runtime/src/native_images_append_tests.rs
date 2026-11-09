//! CPU owned media + existing Root issuer; projection is not authority/admission.
use super::*;
use conduit_resource_snapshot::append::RecordResidenceId;
use std::{cell::Cell, rc::Rc};
struct Storage {
    reads: Rc<Cell<u32>>,
}
impl NorStorage for Storage {
    fn capacity(&self) -> u32 {
        1024
    }
    fn program_unit(&self) -> u32 {
        256
    }
    fn read(
        &mut self,
        _: u32,
        out: &mut [u8],
    ) -> Result<(), conduit_resource_snapshot::nor::NorRefusal> {
        self.reads.set(self.reads.get() + 1);
        out.fill(255);
        Ok(())
    }
    fn program(
        &mut self,
        _: u32,
        _: &[u8],
    ) -> Result<(), conduit_resource_snapshot::nor::NorRefusal> {
        panic!("metadata cannot program")
    }
}
fn identity(actual: &c::HostAdvertisement) -> AppendNativeIdentity {
    AppendNativeIdentity {
        host_id: actual.host_id.clone(),
        boot_id: actual.boot_id.clone(),
        base_id: "append/base".into(),
        provider_instance_id: "append/instance".into(),
        provider_generation: 1,
        resource_pool_id: "append/pool".into(),
        resource_generation_id: c::ResourceGenerationId("append/generation".into()),
        envelope_id: "append/window".into(),
        artifact_id: "image/1".into(),
        implementation_id: "append/implementation".into(),
        base_implementation_id: "append/base-implementation".into(),
    }
}
fn residence(reads: Rc<Cell<u32>>) -> PreparedNorRecordResidence<Storage> {
    PreparedNorRecordResidence::prepare(Storage { reads }, RecordResidenceId([7; 32]), 0, 2)
        .unwrap_or_else(|_| panic!("geometry"))
}
fn ready(
    id: AppendNativeIdentity,
    storage: PreparedNorRecordResidence<Storage>,
    actual: &c::HostAdvertisement,
) -> ReadyRecordAppender<Storage> {
    use crate::execution_authority::{Issuer, NativeFacts, Operation};
    use conduit_body as b;
    use conduit_resource_snapshot::native_append::*;
    let (mut bootstrap, _, _, _) = crate::membership_tests::fixture();
    let credential = crate::membership_tests::receipt(&bootstrap, actual).credential;
    bootstrap.admission = b::MemberBootstrapMaterial::Retained {
        credential,
        secret: [3; 32],
    };
    let preparation = b::HostPreparationDelegation {
        authorizer: b::HostPreparationAuthorizer::SeedOwner,
        maximum_assignment_bytes: 4096,
        maximum_runtime_state_bytes: 4096,
    };
    let execution = b::HostExecutionDelegation {
        authorizer: b::HostExecutionAuthorizer::SeedOwner,
        maximum_active_worksets: 1,
        continuation: None,
    };
    let policy = crate::execution::Policy {
        preparation: Some(&preparation),
        execution: Some(&execution),
        active_worksets: 0,
        maximum_retained_bytes: 4096,
    };
    let context = b::NativePreparationContext {
        bootstrap: &bootstrap,
        member: bootstrap.admission.credential().unwrap(),
        actual,
        image_id: "image/1",
        configuration_id: "configuration/1",
        now_millis: 20,
    };
    let op = Operation {
        kind: APPEND_KIND,
        authority: APPEND_AUTHORITY,
        call: APPEND_OPERATION,
        maximum_parameter_bytes: 200,
        maximum_result_bytes: APPEND_RESULT_BYTES,
        maximum_work_units: 6,
        maximum_operations: 8,
    };
    let mut issuer = Issuer::new();
    // SAFETY: this original CPU-owned recovered physical pair is consumed below.
    let facts = unsafe { NativeFacts::recovered_append(&id, &storage) }.unwrap();
    issuer
        .authorize(context, policy, &facts, &op, "original/append", 4096)
        .unwrap();
    // SAFETY: exact original pair, and the actual original Root Issuer grant.
    unsafe { ReadyRecordAppender::new(id, storage, issuer.grant().unwrap(), 8) }.unwrap()
}
#[test]
fn original_pregrant_and_ready_projection_are_exact_and_do_not_inflate_storage() {
    let (_, mut actual, _, _) = crate::membership_tests::fixture();
    let id = identity(&actual);
    let reads = Rc::new(Cell::new(0));
    let mut storage = residence(reads.clone());
    storage.recover().unwrap();
    let scanned = reads.get();
    // SAFETY: actual original owned CPU pair, consumed unchanged by ready().
    let before = unsafe { recovered_record_append(&id, &storage, &actual, 4096) }.unwrap();
    let owner = ready(id, storage, &actual);
    assert!(owner.recovered());
    let after = record_append(&owner, &actual, 4096).unwrap();
    assert_eq!(before.base, after.base);
    assert_eq!(before.resource, after.resource);
    assert_eq!(before.owned_bytes, after.owned_bytes);
    assert_eq!(after.base.mechanism_family.as_str(), APPEND_MECHANISM);
    assert_eq!(after.resource.class_id.as_str(), APPEND_CLASS);
    assert_eq!(after.resource.capacity_units, 1);
    assert!(after.resource.content.is_none());
    assert!(after.resource.compute.is_none());
    assert_eq!(owner.attachment().resource_bytes, 1024);
    assert_eq!(owner.attachment().slots, 2);
    let mut publication = Publication::new(&mut actual, 1, 4096).unwrap();
    publication.record_append(&owner).unwrap();
    publication.finish().unwrap();
    assert_eq!(actual.bases.len(), 1);
    assert_eq!(actual.resources.len(), 1);
    assert!(actual.capabilities.is_empty());
    assert_eq!(reads.get(), scanned);
}
#[test]
fn pregrant_requires_actual_recovery_context_unique_instance_and_finite_budget() {
    let (_, actual, _, _) = crate::membership_tests::fixture();
    let id = identity(&actual);
    let reads = Rc::new(Cell::new(0));
    let mut storage = residence(reads.clone());
    // SAFETY: original owned CPU pair; unrecovered state must refuse metadata.
    assert!(matches!(
        unsafe { recovered_record_append(&id, &storage, &actual, 4096) },
        Err(Refusal::Context)
    ));
    assert_eq!(reads.get(), 0);
    storage.recover().unwrap();
    let scanned = reads.get();
    // SAFETY: all cases borrow the same original recovered CPU-owned pair.
    assert!(matches!(
        unsafe { recovered_record_append(&id, &storage, &actual, 0) },
        Err(Refusal::Capacity)
    ));
    let mut wrong = actual.clone();
    wrong.boot_id = "foreign/boot".into();
    assert!(matches!(
        unsafe { recovered_record_append(&id, &storage, &wrong, 4096) },
        Err(Refusal::Context)
    ));
    let projected = unsafe { recovered_record_append(&id, &storage, &actual, 4096) }.unwrap();
    let mut duplicate = actual.clone();
    duplicate.resources.push(projected.resource.clone());
    assert!(matches!(
        unsafe { recovered_record_append(&id, &storage, &duplicate, 4096) },
        Err(Refusal::Context)
    ));
    duplicate = actual.clone();
    let mut base = projected.base;
    base.base_id = "different/base".into();
    duplicate.bases.push(base);
    assert!(matches!(
        unsafe { recovered_record_append(&id, &storage, &duplicate, 4096) },
        Err(Refusal::Context)
    ));
    let mut published = actual.clone();
    let mut publication = Publication::new(&mut published, 1, 4096).unwrap();
    unsafe { publication.recovered_record_append(&id, &storage) }.unwrap();
    publication.finish().unwrap();
    assert_eq!(published.resources[0].capacity_units, 1);
    assert!(published.capabilities.is_empty());
    assert_eq!(reads.get(), scanned);
}
