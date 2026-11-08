use super::*;
struct Clock;
impl conduit_monotonic_wait::MonotonicWaitProvider for Clock {
    async fn wait(&mut self, _: u64) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
        Ok(())
    }
}
#[test]
fn historical_seed_basis_describes_current_provider_but_cannot_select_execution() {
    let (mut bootstrap, actual, _, _) = crate::membership_tests::fixture();
    let mut retained = crate::membership_tests::receipt(&bootstrap, &actual).credential;
    retained.boot_id = "previous/boot".into();
    bootstrap.admission = b::MemberBootstrapMaterial::Retained {
        credential: retained,
        secret: [3; 32],
    };
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
    // SAFETY: this test owns its actual finite in-process clock provider.
    // It performs no GPIO and no supplied metadata substitutes for a Back.
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
    let facts = NativeFacts::wait(&clock);
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
    let policy = super::super::execution::Policy {
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
        kind: conduit_time::TICK_KIND,
        authority: conduit_monotonic_wait::WAIT_AUTHORITY,
        call: c::WAIT_HOST_CALL_CONTRACT,
        maximum_parameter_bytes: 8,
        maximum_result_bytes: 8,
        maximum_work_units: 1,
        maximum_operations: 1,
    };
    let mut issuer = Issuer::new();
    issuer
        .authorize(context, policy, &facts, &operation, "original/clock", 4096)
        .unwrap();
    assert_eq!(issuer.grant().unwrap().boot_id, actual.boot_id);
    let original = issuer.grant().unwrap().grant_id.clone();
    issuer
        .authorize(context, policy, &facts, &operation, "original/clock", 4096)
        .unwrap();
    assert_eq!(issuer.grant().unwrap().grant_id, original);
    assert!(matches!(
        issuer.authorize(context, policy, &facts, &operation, "different/clock", 4096),
        Err(Refusal::Scope)
    ));
    assert_eq!(issuer.grant().unwrap().grant_id, original);

    // This is the mandatory first guard used by opaque issue(); historical
    // return evidence is not the fresh current-Boot membership it requires.
    assert!(matches!(
        selected(context, policy, &facts),
        Err(Refusal::Scope)
    ));
    let mut unrelated = context.member.clone();
    unrelated.issued_at_millis += 1;
    let changed = b::NativePreparationContext {
        member: &unrelated,
        ..context
    };
    assert!(matches!(
        selected_description(changed, policy, &facts),
        Err(Refusal::Scope)
    ));
    assert!(matches!(
        selected(changed, policy, &facts),
        Err(Refusal::Scope)
    ));
}
