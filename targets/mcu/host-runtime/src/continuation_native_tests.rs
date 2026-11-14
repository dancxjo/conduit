//! Real signed intent and owned CPU clock; no fabricated current membership.
use super::*;
use ed25519_dalek::Signer;
struct Clock;
impl conduit_monotonic_wait::MonotonicWaitProvider for Clock {
    async fn wait(&mut self, _: u64) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
        Ok(())
    }
}
#[test]
fn signed_cross_boot_intent_selects_actual_native_description_only_with_opt_in() {
    let (mut bootstrap, actual, _, owner) = crate::membership_tests::fixture();
    let mut old = crate::membership_tests::receipt(&bootstrap, &actual).credential;
    old.boot_id = "previous/boot".into();
    bootstrap.admission = b::MemberBootstrapMaterial::Retained {
        credential: old,
        secret: [3; 32],
    };
    let ceiling = b::ContinuationCeilings {
        assignment_bytes: 4096,
        runtime_bytes: 4096,
        durable_bytes: 4096,
        operation_slots: 4,
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
    // Explicit CPU receiver facts, not a claim of authenticated durable media.
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
        maximum_runtime_state_bytes: 1024,
    };
    let execution = b::HostExecutionDelegation {
        authorizer: b::HostExecutionAuthorizer::SeedOwner,
        maximum_active_worksets: 1,
        continuation: Some(ceiling),
    };
    let policy = Policy {
        preparation: Some(&preparation),
        execution: Some(&execution),
        active_worksets: 0,
        maximum_retained_bytes: 4096,
    };
    let mut undersized = execution;
    undersized.continuation.as_mut().unwrap().runtime_bytes -= 1;
    assert!(ContinuationNativeContext::check_installation(
        &bootstrap,
        &actual,
        &verified,
        "image/1",
        "configuration/1",
        Policy {
            execution: Some(&undersized),
            ..policy
        }
    )
    .is_err());
    let context = ContinuationNativeContext::check_installation(
        &bootstrap,
        &actual,
        &verified,
        "image/1",
        "configuration/1",
        policy,
    )
    .unwrap();
    assert_eq!(
        context.verified_intent().actual_boot(),
        actual.boot_id.as_str()
    );
    assert_ne!(
        bootstrap.admission.credential().unwrap().boot_id,
        actual.boot_id
    );
    let identity = conduit_monotonic_wait::WaitNativeIdentity {
        host_id: actual.host_id.clone(),
        boot_id: actual.boot_id.clone(),
        base_id: "clock/base".into(),
        provider_instance_id: "clock/instance".into(),
        provider_generation: 1,
        resource_pool_id: "clock/pool".into(),
        resource_generation_id: c::ResourceGenerationId("clock/generation".into()),
        envelope_id: "clock/envelope".into(),
        artifact_id: "image/1".into(),
        implementation_id: "clock/implementation".into(),
        base_implementation_id: "clock/implementation".into(),
    };
    // SAFETY: the CPU test exclusively owns this finite clock provider.
    let clock = unsafe {
        conduit_monotonic_wait::ReadyMonotonicWaitBase::new(
            identity,
            conduit_monotonic_wait::WaitAttachment {
                generation: 1,
                resource_bytes: 1,
                maximum_duration_ms: 100,
            },
            Clock,
        )
    }
    .unwrap();
    let facts = crate::execution_authority::NativeFacts::wait(&clock);
    let operation = crate::execution_authority::Operation {
        kind: conduit_time::TICK_KIND,
        authority: conduit_monotonic_wait::WAIT_AUTHORITY,
        call: c::WAIT_HOST_CALL_CONTRACT,
        maximum_parameter_bytes: 8,
        maximum_result_bytes: 8,
        maximum_work_units: 1,
        maximum_operations: 1,
    };
    let mut issuer = crate::execution_authority::Issuer::new();
    issuer
        .authorize_continuation(context, policy, &facts, &operation, "original/clock", 4096)
        .unwrap();
    assert_eq!(issuer.grant().unwrap().boot_id, actual.boot_id);
    let original = issuer.grant().unwrap().grant_id.clone();
    issuer
        .authorize_continuation(context, policy, &facts, &operation, "original/clock", 4096)
        .unwrap();
    assert_eq!(issuer.grant().unwrap().grant_id, original);
    // This description creates neither an opaque fresh scope nor a capability
    // table; issue_continuation still requires the unforgeable canonical proof.

    // Reuse the actual initialized clock to publish independently owned Base
    // facts, then request only a descriptive fresh binding (never a table).
    let mut described_actual = actual.clone();
    let mut publication =
        crate::native_images::Publication::new(&mut described_actual, 1, 4096).unwrap();
    publication.wait(&clock).unwrap();
    publication.finish().unwrap();
    let described_context = ContinuationNativeContext::check_installation(
        &bootstrap,
        &described_actual,
        &verified,
        "image/1",
        "configuration/1",
        policy,
    )
    .unwrap();
    let id = c::AssignedIdentity::from_text;
    let binding = c::AssignedNativeBinding {
        capability: id(conduit_monotonic_wait::WAIT_CAPABILITY),
        kind_revision: id("checked/revision"),
        execution_profile: id("checked/profile"),
        maximum_instances: 1,
        queue_items: 1,
        queue_bytes: 8,
        base: id("clock/base"),
        instance: id("calibration/instance"),
        generation: 1,
        base_implementation: id("clock/implementation"),
        mechanism: id(conduit_monotonic_wait::WAIT_ATTACHMENT_CLASS),
        enforcement: 0,
        grant: id("calibration/grant"),
        authority: id(operation.authority),
        call: id(operation.call),
        subject: id(operation.kind),
        host: id("calibration/host"),
        boot: id("calibration/boot"),
        authority_capability: id(conduit_monotonic_wait::WAIT_CAPABILITY),
        resource: 0,
        pool: id("calibration/pool"),
        resource_class: id(conduit_monotonic_wait::WAIT_ATTACHMENT_CLASS),
        units: 1,
    };
    let fresh = issuer
        .describe_continuation(described_context, policy, &facts, &operation, binding)
        .unwrap();
    assert_eq!(fresh.binding.instance, id("clock/instance"));
    assert_eq!(fresh.binding.host, id(actual.host_id.as_str()));
    assert_eq!(fresh.binding.boot, id(actual.boot_id.as_str()));
    assert_eq!(fresh.binding.pool, id("clock/pool"));
    assert_eq!(fresh.grant, original.as_str());
    let mut wrong = binding;
    wrong.base = id("foreign/base");
    assert!(issuer
        .describe_continuation(described_context, policy, &facts, &operation, wrong)
        .is_err());
    let mut changed_operation = operation;
    changed_operation.maximum_operations += 1;
    assert!(issuer
        .describe_continuation(
            described_context,
            policy,
            &facts,
            &changed_operation,
            binding
        )
        .is_err());
    assert_eq!(
        issuer.grant().unwrap().grant_id,
        original,
        "description cannot replace original authority"
    );

    let disabled = b::HostExecutionDelegation {
        continuation: None,
        ..execution
    };
    assert!(ContinuationNativeContext::check_installation(
        &bootstrap,
        &actual,
        &verified,
        "image/1",
        "configuration/1",
        Policy {
            execution: Some(&disabled),
            ..policy
        }
    )
    .is_err());
    let mut changed = actual.clone();
    changed.boot_id = "different/boot".into();
    assert!(ContinuationNativeContext::check_installation(
        &bootstrap,
        &changed,
        &verified,
        "image/1",
        "configuration/1",
        policy
    )
    .is_err());
    let narrow = b::HostExecutionDelegation {
        continuation: Some(b::ContinuationCeilings {
            operation_slots: 1,
            ..ceiling
        }),
        ..execution
    };
    assert!(ContinuationNativeContext::check_installation(
        &bootstrap,
        &actual,
        &verified,
        "image/1",
        "configuration/1",
        Policy {
            execution: Some(&narrow),
            ..policy
        }
    )
    .is_err());
    assert!(ContinuationNativeContext::check_installation(
        &bootstrap,
        &actual,
        &verified,
        "image/1",
        "configuration/1",
        Policy {
            active_worksets: 1,
            ..policy
        }
    )
    .is_err());
}
