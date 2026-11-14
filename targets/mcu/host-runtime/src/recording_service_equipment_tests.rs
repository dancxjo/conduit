//! Actual CPU NOR recovery and original Root Issuers; no device mutations.
use super::*;
use conduit_body as b;
use conduit_resource_snapshot::{
    append::{DurableRecordIndex, RecordRead, RECORD_SLOT_BYTES},
    native_service::SERVICE_CAPABILITY,
    nor::NorRefusal,
};
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
    drops: Rc<Cell<u32>>,
}
impl Storage {
    fn new() -> Self {
        Self {
            bytes: Rc::new(RefCell::new(vec![255; 16384])),
            reads: Rc::new(Cell::new(0)),
            programs: Rc::new(Cell::new(0)),
            drops: Rc::new(Cell::new(0)),
        }
    }
}
impl Drop for Storage {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
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
        for (old, new) in self.bytes.borrow_mut()[offset as usize..offset as usize + input.len()]
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
        base_id: "service/base".into(),
        provider_instance_id: "service/instance".into(),
        provider_generation: 1,
        resource_pool_id: "service/pool".into(),
        resource_generation_id: c::ResourceGenerationId("service/generation".into()),
        envelope_id: "service/envelope".into(),
        artifact_id: "image/1".into(),
        implementation_id: "service/implementation".into(),
        base_implementation_id: "service/implementation".into(),
    }
}
fn selected(
    storage: Storage,
    actual: &c::HostAdvertisement,
    body: &b::BodyId,
) -> Equipment<Storage> {
    // SAFETY: sole actual CPU storage owner, finite buffers, no asynchronous IO.
    unsafe {
        Equipment::new(
            storage,
            identity(actual),
            body,
            &[(0, conduit_resource_snapshot::service_contract::SERVICE_KIND)],
            RESERVED_PREFIX_BYTES,
            16,
        )
    }
}
fn operations() -> [Operation<'static>; 2] {
    [
        Operation {
            kind: APPEND_KIND,
            authority: APPEND_AUTHORITY,
            call: APPEND_OPERATION,
            maximum_parameter_bytes: 200,
            maximum_result_bytes: APPEND_RESULT_BYTES,
            maximum_work_units: 6,
            maximum_operations: 8,
        },
        Operation {
            kind: conduit_resource_snapshot::read_contract::READ_KIND,
            authority: READ_AUTHORITY,
            call: READ_OPERATION,
            maximum_parameter_bytes: conduit_resource_snapshot::read_contract::READ_INPUT_BYTES,
            maximum_result_bytes: conduit_resource_snapshot::read_result::READ_RESULT_BYTES as u32,
            maximum_work_units: 2,
            maximum_operations: 17,
        },
    ]
}
fn with_context(run: impl FnOnce(b::NativePreparationContext<'_>, Policy<'_>)) {
    let (mut bootstrap, actual, _, _) = crate::membership_tests::fixture();
    let credential = crate::membership_tests::receipt(&bootstrap, &actual).credential;
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
    run(
        b::NativePreparationContext {
            bootstrap: &bootstrap,
            member: bootstrap.admission.credential().unwrap(),
            actual: &actual,
            image_id: "image/1",
            configuration_id: "configuration/1",
            now_millis: 20,
        },
        Policy {
            preparation: Some(&preparation),
            execution: Some(&execution),
            active_worksets: 0,
            maximum_retained_bytes: 8192,
        },
    );
}
#[test]
fn selection_recovery_and_publication_preserve_one_real_residence_and_gaps() {
    let (bootstrap, actual, _, _) = crate::membership_tests::fixture();
    let storage = Storage::new();
    let reads = storage.reads.clone();
    // SAFETY: sole CPU media; append-only selection must not select a service.
    let unused = unsafe {
        Equipment::new(
            storage,
            identity(&actual),
            &bootstrap.body_id,
            &[(0, APPEND_KIND)],
            RESERVED_PREFIX_BYTES,
            16,
        )
    };
    assert!(!unused.selected());
    assert!(unused.unused_storage().is_some());
    assert_eq!(reads.get(), 0);
    let mut first = selected(Storage::new(), &actual, &bootstrap.body_id);
    let Some(Stage::Recovered { mut provider, .. }) = first.stage.take() else {
        panic!("recovered")
    };
    provider.append(b"real immutable payload").unwrap();
    let residence = provider.residence_id();
    let storage = provider.into_storage();
    // Model a torn occupied next physical slot; retain that genuine scan evidence.
    storage.bytes.borrow_mut()[(RESERVED_PREFIX_BYTES + RECORD_SLOT_BYTES) as usize] = 0;
    let programs = storage.programs.clone();
    let programmed = programs.get();
    let drops = storage.drops.clone();
    let mut recovered = selected(storage, &actual, &bootstrap.body_id);
    let summary = recovered.recovery().unwrap().as_ref().unwrap();
    assert_eq!(
        (
            summary.committed,
            summary.unpublished_or_damaged,
            summary.next_index,
            summary.total_slots
        ),
        (1, 1, 2, 16)
    );
    let Some(Stage::Recovered { provider, .. }) = recovered.stage.as_mut() else {
        panic!("same provider")
    };
    assert_eq!(provider.residence_id(), residence);
    assert!(
        matches!(provider.read(DurableRecordIndex(0)),Ok(RecordRead::Committed{payload,..}) if payload==b"real immutable payload")
    );
    let mut advertised = actual.clone();
    advertised.bases.clear();
    advertised.resources.clear();
    let mut publication = crate::native_images::Publication::new(&mut advertised, 1, 4096).unwrap();
    recovered.publish(&mut publication).unwrap();
    publication.finish().unwrap();
    assert_eq!(advertised.bases.len(), 1);
    assert_eq!(advertised.resources.len(), 1);
    assert_eq!(
        advertised.bases[0].capability_ids[0].as_str(),
        SERVICE_CAPABILITY
    );
    assert_eq!(
        advertised.resources[0].class_id.as_str(),
        conduit_resource_snapshot::native_append::APPEND_CLASS
    );
    assert_eq!(advertised.resources[0].capacity_units, 1);
    assert!(advertised.resources[0].content.is_none());
    assert!(recovered.ready().is_none());
    assert!(recovered.ready_slot().is_none());
    assert_eq!(programs.get(), programmed);
    assert_eq!(drops.get(), 0);
    // Duplicate publication of the same physical residence cannot invent capacity.
    {
        let mut duplicate =
            crate::native_images::Publication::new(&mut advertised, 1, 4096).unwrap();
        assert!(recovered.publish(&mut duplicate).is_err());
    }
    assert_eq!(advertised.resources.len(), 1);
}
#[test]
fn two_original_issuers_install_idempotently_and_never_masquerade_as_append() {
    with_context(|context, policy| {
        let storage = Storage::new();
        let reads = storage.reads.clone();
        let programs = storage.programs.clone();
        let drops = storage.drops.clone();
        let mut equipment = selected(storage, context.actual, &context.bootstrap.body_id);
        let scanned = reads.get();
        let [append, read] = operations();
        let mut append_issuer = Issuer::new();
        let mut read_issuer = Issuer::new();
        assert!(matches!(
            equipment.authorize(
                context,
                policy,
                [&mut append_issuer, &mut read_issuer],
                [&read, &append],
                4096
            ),
            Err(Refusal::Scope)
        ));
        assert!(append_issuer.grant().is_none() && read_issuer.grant().is_none());
        assert!(equipment.ready().is_none());
        equipment
            .authorize(
                context,
                policy,
                [&mut append_issuer, &mut read_issuer],
                [&append, &read],
                4096,
            )
            .unwrap();
        let ready = equipment.ready().unwrap();
        assert_eq!(
            ready.limits(),
            ServiceLimits {
                append_operations: 8,
                read_operations: 17
            }
        );
        assert_eq!(&ready.original_grants()[0], append_issuer.grant().unwrap());
        assert_eq!(&ready.original_grants()[1], read_issuer.grant().unwrap());
        assert_ne!(
            ready.original_grants()[0].grant_id,
            ready.original_grants()[1].grant_id
        );
        assert_eq!(
            ready.original_grants()[0].subject_kind.as_str(),
            APPEND_KIND
        );
        assert_eq!(ready.original_grants()[1].subject_kind.as_str(), read.kind);
        for grant in ready.original_grants() {
            assert_eq!(grant.capability_id.as_str(), SERVICE_CAPABILITY);
            assert_eq!(grant.host_id, context.actual.host_id);
            assert_eq!(grant.boot_id, context.actual.boot_id);
        }
        let address = equipment.ready().unwrap() as *const _;
        equipment
            .authorize(
                context,
                policy,
                [&mut append_issuer, &mut read_issuer],
                [&append, &read],
                4096,
            )
            .unwrap();
        assert_eq!(equipment.ready().unwrap() as *const _, address);
        assert!(equipment.ready_slot().unwrap().as_ref().is_some());
        assert_eq!(reads.get(), scanned);
        assert_eq!(programs.get(), 0);
        assert_eq!(drops.get(), 0);
        let mut published = context.actual.clone();
        published.bases.clear();
        published.resources.clear();
        let mut publication =
            crate::native_images::Publication::new(&mut published, 1, 4096).unwrap();
        equipment.publish(&mut publication).unwrap();
        publication.finish().unwrap();
        assert_eq!(published.resources[0].capacity_units, 1);
        assert_eq!(
            published.bases[0].capability_ids[0].as_str(),
            SERVICE_CAPABILITY
        );
    });
}
#[test]
fn second_original_refusal_preserves_first_original_and_recovered_owner_for_exact_retry() {
    with_context(|context, policy| {
        let storage = Storage::new();
        let drops = storage.drops.clone();
        let programs = storage.programs.clone();
        let mut equipment = selected(storage, context.actual, &context.bootstrap.body_id);
        let [append, mut read] = operations();
        let mut append_issuer = Issuer::new();
        let mut read_issuer = Issuer::new();
        read_issuer
            .authorize(
                context,
                policy,
                &equipment.facts().unwrap(),
                &read,
                ORIGINALS[1],
                4096,
            )
            .unwrap();
        read.maximum_operations = 18;
        assert!(matches!(
            equipment.authorize(
                context,
                policy,
                [&mut append_issuer, &mut read_issuer],
                [&append, &read],
                4096
            ),
            Err(Refusal::Authority { operation: 1, .. })
        ));
        assert!(append_issuer.grant().is_some());
        assert!(read_issuer.grant().is_some());
        assert!(equipment.ready().is_none());
        assert!(matches!(equipment.stage, Some(Stage::Recovered { .. })));
        assert_eq!(drops.get(), 0);
        assert_eq!(programs.get(), 0);
        read.maximum_operations = 17;
        equipment
            .authorize(
                context,
                policy,
                [&mut append_issuer, &mut read_issuer],
                [&append, &read],
                4096,
            )
            .unwrap();
        assert!(equipment.ready_slot().unwrap().as_ref().is_some());
    });
}
#[test]
fn grant_copy_budget_and_retained_install_refusals_preserve_actual_recovered_pair() {
    with_context(|context, policy| {
        let storage = Storage::new();
        let drops = storage.drops.clone();
        let programs = storage.programs.clone();
        let mut equipment = selected(storage, context.actual, &context.bootstrap.body_id);
        let [append, read] = operations();
        let mut append_issuer = Issuer::new();
        let mut read_issuer = Issuer::new();
        for (i, issuer) in [&mut append_issuer, &mut read_issuer]
            .into_iter()
            .enumerate()
        {
            issuer
                .authorize(
                    context,
                    policy,
                    &equipment.facts().unwrap(),
                    [&append, &read][i],
                    ORIGINALS[i],
                    4096,
                )
                .unwrap();
        }
        let short = append_issuer
            .retained_backing_bytes()
            .unwrap()
            .max(read_issuer.retained_backing_bytes().unwrap());
        assert!(equipment
            .authorize(
                context,
                policy,
                [&mut append_issuer, &mut read_issuer],
                [&append, &read],
                short
            )
            .is_err());
        assert!(equipment.ready().is_none());
        assert!(matches!(equipment.stage, Some(Stage::Recovered { .. })));
        assert_eq!(drops.get(), 0);
        // Real original grants remain scoped to the actual Boot. Deliberately wrong
        // identity at the retained constructor must return the same physical provider.
        let Some(Stage::Recovered { identity, .. }) = equipment.stage.as_mut() else {
            panic!("recovered")
        };
        identity.boot_id = "wrong/boot".into();
        assert!(matches!(
            equipment.install([&append_issuer, &read_issuer], [&append, &read], 4096),
            Err(Refusal::Installation(ServiceRefusal::WrongBinding))
        ));
        let Some(Stage::Recovered { identity, .. }) = equipment.stage.as_mut() else {
            panic!("owner preserved")
        };
        identity.boot_id = context.actual.boot_id.clone();
        equipment
            .authorize(
                context,
                policy,
                [&mut append_issuer, &mut read_issuer],
                [&append, &read],
                4096,
            )
            .unwrap();
        assert!(equipment.ready_slot().unwrap().as_ref().is_some());
        assert_eq!(programs.get(), 0);
        assert_eq!(drops.get(), 0);
    });
}

#[path = "recording_service_binding_tests.rs"]
mod binding;
