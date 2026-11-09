//! Owned CPU media tests, not physical admission or execution-window evidence.
use super::*;
use conduit_body as b;
use conduit_resource_snapshot::{append::RECORD_SLOT_BYTES, nor::NorRefusal};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    vec,
    vec::Vec,
};
struct Storage {
    bytes: Rc<RefCell<Vec<u8>>>,
    reads: Rc<Cell<u32>>,
    programs: Rc<Cell<u32>>,
}
impl Storage {
    fn new() -> Self {
        Self {
            bytes: Rc::new(RefCell::new(vec![255; 16384])),
            reads: Rc::new(Cell::new(0)),
            programs: Rc::new(Cell::new(0)),
        }
    }
}
impl NorStorage for Storage {
    fn capacity(&self) -> u32 {
        self.bytes.borrow().len() as u32
    }
    fn program_unit(&self) -> u32 {
        256
    }
    fn read(&mut self, offset: u32, out: &mut [u8]) -> Result<(), NorRefusal> {
        assert!(offset >= RESERVED_PREFIX_BYTES);
        self.reads.set(self.reads.get() + 1);
        out.copy_from_slice(&self.bytes.borrow()[offset as usize..offset as usize + out.len()]);
        Ok(())
    }
    fn program(&mut self, offset: u32, input: &[u8]) -> Result<(), NorRefusal> {
        assert!(offset >= RESERVED_PREFIX_BYTES);
        self.programs.set(self.programs.get() + 1);
        let mut bytes = self.bytes.borrow_mut();
        for (old, new) in bytes[offset as usize..offset as usize + input.len()]
            .iter_mut()
            .zip(input)
        {
            assert_eq!(*old & new, *new);
            *old = *new;
        }
        Ok(())
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
        base_implementation_id: "append/implementation".into(),
    }
}
fn selected(
    storage: Storage,
    actual: &c::HostAdvertisement,
    body: &conduit_body::BodyId,
) -> Equipment<Storage> {
    let storage_capacity = storage.capacity();
    // SAFETY: actual exclusively owned CPU media, no asynchronous physical IO.
    unsafe {
        Equipment::new(
            storage,
            identity(actual),
            body,
            &[(0, APPEND_KIND)],
            RESERVED_PREFIX_BYTES,
            (storage_capacity - RESERVED_PREFIX_BYTES) / RECORD_SLOT_BYTES,
        )
    }
}
#[test]
fn unselected_preserves_unused_flash_and_selected_recovery_preserves_prefix() {
    let (bootstrap, actual, _, _) = crate::membership_tests::fixture();
    let storage = Storage::new();
    let reads = storage.reads.clone();
    let programs = storage.programs.clone();
    // SAFETY: original CPU owner, no effects selected.
    let unused = unsafe {
        Equipment::new(
            storage,
            identity(&actual),
            &bootstrap.body_id,
            &[],
            RESERVED_PREFIX_BYTES,
            1,
        )
    };
    assert!(!unused.selected());
    assert!(unused.unused_storage().is_some());
    assert!(unused.held_pool().is_none());
    assert!(unused.recovery().is_none());
    assert_eq!(reads.get(), 0);
    assert_eq!(programs.get(), 0);
    let selected_storage = Storage::new();
    let selected_reads = selected_storage.reads.clone();
    let mut selected = selected(selected_storage, &actual, &bootstrap.body_id);
    let summary = selected.recovery().unwrap().as_ref().unwrap();
    assert_eq!(summary.total_slots, 16);
    assert_eq!(summary.next_index, 0);
    assert!(selected.ready_slot().is_none());
    assert!(selected.held_pool().is_some());
    if let Some(Stage::Recovered { provider, .. }) = selected.stage.as_ref() {
        assert_eq!(provider.residence_bytes(), 8192);
        assert_eq!(provider.maximum_recovery_reads(), 32);
        assert_eq!(selected_reads.get(), 32);
    }
}
#[test]
fn reboot_preserves_identity_and_foreign_body_refuses_without_writes_or_owner_loss() {
    let (bootstrap, mut actual, _, _) = crate::membership_tests::fixture();
    let mut first = selected(Storage::new(), &actual, &bootstrap.body_id);
    let Some(Stage::Recovered { mut provider, .. }) = first.stage.take() else {
        panic!("recovered");
    };
    provider.append(b"opaque immutable record").unwrap();
    let residence = provider.residence_id();
    let storage = provider.into_storage();
    let programs = storage.programs.clone();
    let programmed = programs.get();
    actual.boot_id = "fresh/boot".into();
    let mut resumed = selected(storage, &actual, &bootstrap.body_id);
    assert_eq!(resumed.recovery().unwrap().as_ref().unwrap().committed, 1);
    let Some(Stage::Recovered { provider, .. }) = resumed.stage.take() else {
        panic!("recovered");
    };
    assert_eq!(provider.residence_id(), residence);
    let foreign: conduit_body::BodyId = serde_json::from_str(r#""foreign/body""#).unwrap();
    let mut refused = selected(provider.into_storage(), &actual, &foreign);
    assert!(matches!(
        refused.recovery(),
        Some(Err(RecordResidenceRefusal::ForeignResidence))
    ));
    assert!(refused.ready_slot().is_none());
    assert!(refused.facts().is_err());
    assert!(matches!(refused.stage, Some(Stage::Recovered { .. })));
    assert_eq!(programs.get(), programmed);
}
#[test]
fn original_issuer_transition_is_idempotent_and_retained_constructor_returns_owners() {
    let (mut bootstrap, actual, _, _) = crate::membership_tests::fixture();
    let retained = crate::membership_tests::receipt(&bootstrap, &actual).credential;
    bootstrap.admission = b::MemberBootstrapMaterial::Retained {
        credential: retained,
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
    let policy = Policy {
        preparation: Some(&preparation),
        execution: Some(&execution),
        active_worksets: 0,
        maximum_retained_bytes: 4096,
    };
    let context = b::NativePreparationContext {
        bootstrap: &bootstrap,
        member: bootstrap.admission.credential().unwrap(),
        actual: &actual,
        image_id: "image/1",
        configuration_id: "configuration/1",
        now_millis: 20,
    };
    let operation = Operation {
        kind: APPEND_KIND,
        authority: APPEND_AUTHORITY,
        call: APPEND_OPERATION,
        maximum_parameter_bytes: 200,
        maximum_result_bytes: APPEND_RESULT_BYTES,
        maximum_work_units: 6,
        maximum_operations: 8,
    };
    let storage = Storage::new();
    let reads = storage.reads.clone();
    let programs = storage.programs.clone();
    let mut equipment = selected(storage, &actual, &bootstrap.body_id);
    let scanned = reads.get();
    let mut issuer = Issuer::new();
    equipment
        .authorize(context, policy, &mut issuer, &operation, 4096)
        .unwrap();
    equipment
        .authorize(context, policy, &mut issuer, &operation, 4096)
        .unwrap();
    let owner = equipment.ready_slot().unwrap().take().unwrap();
    assert_eq!(owner.original_grant(), issuer.grant().unwrap());
    assert_eq!(owner.attachment().maximum_operations, 8);
    let original = owner.original_grant().clone();
    let id = identity(&actual);
    let provider = owner.into_provider();
    let mut foreign = original.clone();
    foreign.boot_id = "foreign/boot".into();
    // SAFETY: same original CPU-owned pair; refusal must return both owners.
    let (id, provider, refusal) =
        unsafe { ReadyRecordAppender::new_retained(id, provider, &foreign, 8) }
            .err()
            .unwrap();
    assert_eq!(refusal, AppendCallRefusal::WrongBinding);
    // SAFETY: unchanged original owner pair and exact original Issuer grant.
    let rebuilt = unsafe { ReadyRecordAppender::new_retained(id, provider, &original, 8) }
        .unwrap_or_else(|_| panic!("original pair"));
    assert_eq!(rebuilt.original_grant(), &original);
    assert_eq!(reads.get(), scanned);
    assert_eq!(programs.get(), 0);
}
