//! Actual recovered CPU storage and existing Root authorization policy.
use super::*;
use conduit_resource_snapshot::{append::RecordResidenceId, native_append::*};
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
        panic!("facts/authorization must never program media")
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
fn operation() -> Operation<'static> {
    Operation {
        kind: APPEND_KIND,
        authority: APPEND_AUTHORITY,
        call: APPEND_OPERATION,
        maximum_parameter_bytes: 200,
        maximum_result_bytes: APPEND_RESULT_BYTES,
        maximum_work_units: 6,
        maximum_operations: 8,
    }
}
fn residence(reads: Rc<Cell<u32>>) -> PreparedNorRecordResidence<Storage> {
    PreparedNorRecordResidence::prepare(Storage { reads }, RecordResidenceId([7; 32]), 0, 2)
        .unwrap_or_else(|_| panic!("geometry"))
}
#[test]
fn recovered_original_owner_breaks_grant_cycle_without_changing_ready_fingerprint() {
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
    let policy = crate::execution::Policy {
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
    let id = identity(&actual);
    let reads = Rc::new(Cell::new(0));
    let mut storage = residence(reads.clone());
    let op = operation();
    let mut issuer = Issuer::new();
    // SAFETY: original exclusively owned CPU residence and identity, no GPIO.
    assert!(unsafe { NativeFacts::recovered_append(&id, &storage) }.is_err());
    assert!(issuer.grant().is_none());
    assert_eq!(reads.get(), 0);
    let summary = storage.recover().unwrap();
    assert_eq!(summary.total_slots, 2);
    let scanned = reads.get();
    let fingerprint = {
        // SAFETY: same original recovered storage/identity is consumed below.
        let facts = unsafe { NativeFacts::recovered_append(&id, &storage) }.unwrap();
        issuer
            .authorize(context, policy, &facts, &op, "original/append", 4096)
            .unwrap();
        super::super::fingerprint(context, &facts, &op)
    };
    assert_eq!(reads.get(), scanned);
    // SAFETY: same original pair and the actual original Issuer grant.
    let ready =
        unsafe { ReadyRecordAppender::new(id, storage, issuer.grant().unwrap(), 8) }.unwrap();
    let facts = NativeFacts::append(&ready);
    assert_eq!(super::super::fingerprint(context, &facts, &op), fingerprint);
    issuer
        .authorize(context, policy, &facts, &op, "original/append", 4096)
        .unwrap();
    assert_eq!(ready.attachment().slots, 2);
    assert_eq!(ready.attachment().maximum_operations, 8);
    assert_eq!(reads.get(), scanned);
    let mut wrong = issuer.grant().unwrap().clone();
    wrong.boot_id = "stale/boot".into();
    let id = ready.native_identity().clone();
    let storage = ready.into_provider();
    // SAFETY: original CPU ownership retained; deliberately invalid grant must refuse.
    assert!(matches!(
        unsafe { ReadyRecordAppender::new(id, storage, &wrong, 8) },
        Err(AppendCallRefusal::WrongBinding)
    ));
}
#[test]
fn signed_cross_boot_continuation_authorizes_same_recovered_and_ready_append_facts() {
    use ed25519_dalek::Signer;
    let (mut bootstrap, actual, _, owner) = crate::membership_tests::fixture();
    let mut retained = crate::membership_tests::receipt(&bootstrap, &actual).credential;
    retained.boot_id = "previous/boot".into();
    bootstrap.admission = b::MemberBootstrapMaterial::Retained {
        credential: retained,
        secret: [3; 32],
    };
    let ceiling = b::ContinuationCeilings {
        assignment_bytes: 4096,
        runtime_bytes: 4096,
        durable_bytes: 4096,
        operation_slots: 16,
        active_worksets: 1,
    };
    let intent = b::ContinuationIntent {
        schema: b::CONTINUATION_INTENT_SCHEMA,
        body_id: "body/1",
        part_id: "part/1",
        host_id: "host/1",
        credential_id: "credential/1",
        membership_generation: 3,
        intent_generation: 1,
        issuer_host: "owner/1",
        issuer_key: owner.verifying_key().to_bytes(),
        source_digest: [1; 32],
        workset_digest: [2; 32],
        authority: b::ContinuationAuthority::LocalWorksetReplacement,
        scope_digest: [3; 32],
        image_id: "image/1",
        configuration_id: "configuration/1",
        ceilings: ceiling,
    };
    let signature = owner.sign(&intent.signing_transcript()).to_bytes();
    // CPU receiver facts only; this does not claim physical installed-intent custody.
    let verified = b::verify_continuation_intent(
        intent,
        &signature,
        Some(b::ContinuationPolicy {
            issuer_host: "owner/1",
            issuer_key: owner.verifying_key().to_bytes(),
            maximum: ceiling,
        }),
        b::TrustedInstalledContinuationFacts {
            body_id: "body/1",
            part_id: "part/1",
            host_id: "host/1",
            credential_id: "credential/1",
            membership_generation: 3,
            intent_generation: 1,
            source_digest: [1; 32],
            workset_digest: [2; 32],
            image_id: "image/1",
            configuration_id: "configuration/1",
            actual_boot: actual.boot_id.as_str(),
        },
    )
    .unwrap();
    let preparation = b::HostPreparationDelegation {
        authorizer: b::HostPreparationAuthorizer::SeedOwner,
        maximum_assignment_bytes: 4096,
        maximum_runtime_state_bytes: 4096,
    };
    let execution = b::HostExecutionDelegation {
        authorizer: b::HostExecutionAuthorizer::SeedOwner,
        maximum_active_worksets: 1,
        continuation: Some(ceiling),
    };
    let policy = crate::execution::Policy {
        preparation: Some(&preparation),
        execution: Some(&execution),
        active_worksets: 0,
        maximum_retained_bytes: 4096,
    };
    let context = crate::continuation_native::ContinuationNativeContext::check_installation(
        &bootstrap,
        &actual,
        &verified,
        "image/1",
        "configuration/1",
        policy,
    )
    .unwrap();
    let id = identity(&actual);
    let reads = Rc::new(Cell::new(0));
    let mut storage = residence(reads.clone());
    storage.recover().unwrap();
    let scanned = reads.get();
    let op = operation();
    let mut issuer = Issuer::new();
    {
        // SAFETY: original CPU provider/identity, consumed unchanged below.
        let facts = unsafe { NativeFacts::recovered_append(&id, &storage) }.unwrap();
        issuer
            .authorize_continuation(context, policy, &facts, &op, "original/append", 4096)
            .unwrap();
    }
    let original = issuer.grant().unwrap().clone();
    assert_eq!(original.boot_id, actual.boot_id);
    assert_ne!(
        original.boot_id,
        bootstrap.admission.credential().unwrap().boot_id
    );
    // SAFETY: same original recovered residence and exact Issuer grant.
    let ready =
        unsafe { ReadyRecordAppender::new(id, storage, issuer.grant().unwrap(), 8) }.unwrap();
    issuer
        .authorize_continuation(
            context,
            policy,
            &NativeFacts::append(&ready),
            &op,
            "original/append",
            4096,
        )
        .unwrap();
    assert_eq!(issuer.grant().unwrap(), &original);
    assert_eq!(reads.get(), scanned);
    // Authorization is still only a description; issue_continuation requires
    // the original opaque fresh Body scope and approved fragment, unchanged.
}
