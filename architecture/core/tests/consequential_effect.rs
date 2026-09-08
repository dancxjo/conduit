use conduit_core::*;

struct FixtureProvider {
    physical_state: bool,
    calls: u32,
    safe_calls: u32,
    outcome: Result<PhysicalObservation, ConsequentialProviderFailure>,
}

impl ConsequentialEffectProvider for FixtureProvider {
    fn apply(
        &mut self,
        _request: &ConsequentialRequest,
    ) -> Result<PhysicalObservation, ConsequentialProviderFailure> {
        self.calls += 1;
        let outcome = self.outcome;
        if outcome == Ok(PhysicalObservation::Observed) {
            self.physical_state = true;
        }
        outcome
    }

    fn safe_disposition(&mut self, disposition: &str) -> Result<(), ConsequentialProviderFailure> {
        assert_eq!(disposition, "deenergized");
        self.safe_calls += 1;
        self.physical_state = false;
        Ok(())
    }
}

fn issue() -> CapabilityIssueRequest {
    let operation = HostOperationContractId::from("conduit.host/relay-pulse@1");
    let scope = BaseCapabilityScope {
        host_id: HostId::from("host/bench"),
        boot_id: BootId::from("boot/current"),
        base_instance_id: BaseInstanceId::from("base/relay"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/pulse"),
        active_play_id: ActivePlayId::from("play/pulse"),
        authority_grant_id: AuthorityGrantId::from("grant/pulse"),
        authority_contract_id: AuthorityContractId::from("authority/attended-relay@1"),
        capability_id: CapabilityId::from("relay/pulse"),
        implementation_id: ImplementationId::from("fixture/relay-base@1"),
        operation_contract_id: operation.clone(),
        subject_kind: KindId::from("switch/pulse"),
        resource_pool_id: ResourcePoolId::from("relay/channel-1"),
        resource_generation_id: ResourceGenerationId("relay/channel-1/generation-9".into()),
        envelope_id: CapabilityEnvelopeId::from("relay/pulse/max-10"),
        maximum_parameter_bytes: 24,
        maximum_result_bytes: 1,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    CapabilityIssueRequest {
        authority: BaseCapabilityAuthority {
            grant: AuthorityGrant {
                grant_id: scope.authority_grant_id.clone(),
                contract_id: scope.authority_contract_id.clone(),
                host_operation_contract_id: operation.clone(),
                subject_kind: scope.subject_kind.clone(),
                host_id: scope.host_id.clone(),
                boot_id: scope.boot_id.clone(),
                capability_id: scope.capability_id.clone(),
            },
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: operation,
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: 24,
            maximum_result_bytes: 1,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        },
        scope,
    }
}

fn gate(valid_from: u64, valid_until: u64) -> (ConsequentialEffectGate, AttendanceHandle) {
    let issue = issue();
    let scope = &issue.scope;
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
        parameter_bytes: 24,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [7; 32],
        1,
    )
    .unwrap();
    let capability_handle = table.issue(issue).unwrap();
    let (attendance, attendance_handle) = AttendanceGrant::issue(
        [8; 32],
        1,
        "attendance/relay/1".into(),
        "relay/channel-1".into(),
        9,
        "pulse".into(),
        valid_from,
        valid_until,
    )
    .unwrap();
    (
        ConsequentialEffectGate::new(
            ConsequentialProfile {
                profile_id: "consequence/attended-relay@1".into(),
                requires_attendance: true,
                envelope: ConsequentialEnvelope {
                    maximum_magnitude: 1,
                    maximum_duration_ticks: 10,
                    maximum_rate: 2,
                },
                safe_disposition: "deenergized".into(),
            },
            table,
            capability_handle,
            claim,
            Some(attendance),
            "relay/channel-1".into(),
            9,
            3,
        )
        .unwrap(),
        attendance_handle,
    )
}

fn request() -> ConsequentialRequest {
    ConsequentialRequest {
        resource_id: "relay/channel-1".into(),
        resource_generation: 9,
        operation_id: "pulse".into(),
        magnitude: 1,
        duration_ticks: 5,
        rate: 1,
    }
}

fn ready() -> SafetyReadiness {
    SafetyReadiness {
        resource_id: "relay/channel-1".into(),
        resource_generation: 9,
        observation_generation: 3,
        ready: true,
    }
}

fn provider() -> FixtureProvider {
    FixtureProvider {
        physical_state: false,
        calls: 0,
        safe_calls: 0,
        outcome: Ok(PhysicalObservation::Observed),
    }
}

#[test]
fn attended_last_mile_gate_refuses_before_effect_and_observes_exact_success() {
    let (mut gate, attendance) = gate(10, 20);
    let mut provider = provider();
    assert_eq!(
        gate.attempt(&mut provider, 12, None, &ready(), &request()),
        ConsequentialDisposition::Refused(ConsequentialRefusal::MissingAttendance)
    );
    assert_eq!(provider.calls, 0);
    assert!(!provider.physical_state);

    let mut excessive = request();
    excessive.duration_ticks = 11;
    assert_eq!(
        gate.attempt(&mut provider, 12, Some(&attendance), &ready(), &excessive),
        ConsequentialDisposition::Refused(ConsequentialRefusal::EnvelopeExceeded)
    );
    assert_eq!(provider.calls, 0);

    assert_eq!(
        gate.attempt(&mut provider, 12, Some(&attendance), &ready(), &request()),
        ConsequentialDisposition::PhysicalEffectObserved
    );
    assert_eq!(provider.calls, 1);
    assert!(provider.physical_state);
    assert_eq!(
        gate.attempt(&mut provider, 12, Some(&attendance), &ready(), &request()),
        ConsequentialDisposition::Refused(ConsequentialRefusal::AttendanceReplay)
    );
    assert_eq!(provider.calls, 1);
}

#[test]
fn stale_expired_forged_and_unready_inputs_never_reach_provider() {
    let mut provider = provider();
    let (mut expired, expired_handle) = gate(1, 2);
    assert_eq!(
        expired.attempt(
            &mut provider,
            2,
            Some(&expired_handle),
            &ready(),
            &request()
        ),
        ConsequentialDisposition::Refused(ConsequentialRefusal::AttendanceExpired)
    );

    let (mut unready, handle) = gate(1, 20);
    let mut safety = ready();
    safety.ready = false;
    assert_eq!(
        unready.attempt(&mut provider, 2, Some(&handle), &safety, &request()),
        ConsequentialDisposition::Refused(ConsequentialRefusal::SafetyNotReady)
    );

    let (mut stale, handle) = gate(1, 20);
    let mut wrong = request();
    wrong.resource_generation = 10;
    assert_eq!(
        stale.attempt(&mut provider, 2, Some(&handle), &ready(), &wrong),
        ConsequentialDisposition::Refused(ConsequentialRefusal::ResourceStale)
    );

    let (mut target, _) = gate(1, 20);
    let (_, foreign_handle) = AttendanceGrant::issue(
        [9; 32],
        2,
        "attendance/foreign".into(),
        "relay/channel-1".into(),
        9,
        "pulse".into(),
        1,
        20,
    )
    .unwrap();
    assert_eq!(
        target.attempt(
            &mut provider,
            2,
            Some(&foreign_handle),
            &ready(),
            &request()
        ),
        ConsequentialDisposition::Refused(ConsequentialRefusal::ForgedAttendance)
    );
    assert_eq!(provider.calls, 0);
}

#[test]
fn provider_loss_is_safe_and_ambiguous_failure_is_never_retried() {
    let (mut lost, _) = gate(1, 20);
    let mut provider = provider();
    provider.physical_state = true;
    assert_eq!(lost.lose(&mut provider), ConsequentialDisposition::Safe);
    assert!(!provider.physical_state);
    assert_eq!(provider.safe_calls, 1);

    let (mut ambiguous, attendance) = gate(1, 20);
    provider.outcome = Err(ConsequentialProviderFailure::Ambiguous);
    assert_eq!(
        ambiguous.attempt(&mut provider, 2, Some(&attendance), &ready(), &request()),
        ConsequentialDisposition::Refused(ConsequentialRefusal::Provider(
            ConsequentialProviderFailure::Ambiguous
        ))
    );
    assert_eq!(provider.calls, 1);
    assert_eq!(
        ambiguous.attempt(&mut provider, 2, Some(&attendance), &ready(), &request()),
        ConsequentialDisposition::Refused(ConsequentialRefusal::AttendanceReplay)
    );
    assert_eq!(provider.calls, 1);
}
