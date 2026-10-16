use conduit_process::*;
#[path = "common/print_fixture.rs"]
mod fixture;
use fixture::*;

fn job(id: &str) -> PrintRequest {
    let original = request(PrintMedia::A4, PrintColor::Monochrome, PrintSides::OneSided);
    PrintRequest::new(
        original.attributes().clone(),
        original.document().clone(),
        identity(id),
    )
    .unwrap()
}
#[test]
fn finite_registry_refuses_duplicates_and_overflow_without_forgetting_uncertainty() {
    assert!(matches!(
        PrintAttemptRegistry::<0>::prepare(),
        Err(PrintRegistryRefusal::InvalidCapacity)
    ));
    assert!(matches!(
        PrintAttemptRegistry::<33>::prepare(),
        Err(PrintRegistryRefusal::InvalidCapacity)
    ));
    let mut registry = PrintAttemptRegistry::<2>::prepare().unwrap();
    assert_eq!(registry.capacity(), 2);
    let first = identity("attempt/first");
    registry
        .admit(
            &job("job/first"),
            &support(false),
            &binding(),
            first.clone(),
        )
        .unwrap();
    registry
        .observe(&first, PrintProviderReport::Dispatched)
        .unwrap();
    assert!(matches!(
        registry
            .observe(&first, PrintProviderReport::AcknowledgementUnknown)
            .unwrap(),
        PrintLifecycle::Uncertain(_)
    ));
    assert_eq!(
        registry.admit(
            &job("job/first"),
            &support(false),
            &binding(),
            identity("attempt/new")
        ),
        Err(PrintRegistryRefusal::DuplicateJobId)
    );
    assert_eq!(
        registry.admit(
            &job("job/different"),
            &support(false),
            &binding(),
            first.clone()
        ),
        Err(PrintRegistryRefusal::DuplicateAttemptId)
    );
    registry
        .admit(
            &job("job/second"),
            &support(false),
            &binding(),
            identity("attempt/second"),
        )
        .unwrap();
    assert_eq!(
        registry.admit(
            &job("job/third"),
            &support(false),
            &binding(),
            identity("attempt/third")
        ),
        Err(PrintRegistryRefusal::CapacityExhausted)
    );
    assert_eq!(registry.admitted(), 2);
    assert_eq!(
        registry.attempt(&first).unwrap().stage(),
        PrintStage::Terminal
    );
    assert_eq!(
        registry.observe(&first, PrintProviderReport::Dispatched),
        Err(PrintRegistryRefusal::Protocol(
            PrintProtocolRefusal::InvalidTransition
        ))
    );
    assert_eq!(
        registry.observe(
            &identity("attempt/missing"),
            PrintProviderReport::ProviderLost
        ),
        Err(PrintRegistryRefusal::UnknownAttempt)
    );
}
#[test]
fn cancellation_and_lost_provider_never_reopen_a_job_id() {
    for report in [
        PrintProviderReport::AcknowledgementUnknown,
        PrintProviderReport::ProviderLost,
        PrintProviderReport::CancellationRequested,
    ] {
        let mut registry = PrintAttemptRegistry::<1>::prepare().unwrap();
        let id = identity("attempt/one");
        registry
            .admit(&job("job/one"), &support(false), &binding(), id.clone())
            .unwrap();
        // Exactly one simulated dispatch. Reports are evidence, not print I/O.
        let mut simulated_dispatches = 0;
        if matches!(
            registry
                .observe(&id, PrintProviderReport::Dispatched)
                .unwrap(),
            PrintLifecycle::Dispatched(_)
        ) {
            simulated_dispatches += 1;
        }
        assert!(matches!(
            registry.observe(&id, report).unwrap(),
            PrintLifecycle::Uncertain(_)
        ));
        for _ in 0..4 {
            assert_eq!(
                registry.admit(
                    &job("job/one"),
                    &support(false),
                    &binding(),
                    identity("attempt/retry")
                ),
                Err(PrintRegistryRefusal::DuplicateJobId)
            );
            assert!(registry
                .observe(&id, PrintProviderReport::Dispatched)
                .is_err());
        }
        assert_eq!(simulated_dispatches, 1);
    }
    let mut registry = PrintAttemptRegistry::<1>::prepare().unwrap();
    let id = identity("attempt/cancelled");
    registry
        .admit(
            &job("job/cancelled"),
            &support(false),
            &binding(),
            id.clone(),
        )
        .unwrap();
    assert!(matches!(
        registry
            .observe(&id, PrintProviderReport::CancellationRequested)
            .unwrap(),
        PrintLifecycle::Cancelled(_)
    ));
    assert_eq!(
        registry.admit(
            &job("job/cancelled"),
            &support(false),
            &binding(),
            identity("attempt/after-cancel")
        ),
        Err(PrintRegistryRefusal::DuplicateJobId)
    );
}
#[test]
fn unsupported_admission_does_not_consume_or_claim_an_attempt() {
    let mut registry = PrintAttemptRegistry::<1>::prepare().unwrap();
    let color = request(PrintMedia::A4, PrintColor::Color, PrintSides::OneSided);
    assert_eq!(
        registry.admit(
            &color,
            &support(false),
            &binding(),
            identity("attempt/refused")
        ),
        Err(PrintRegistryRefusal::Admission(
            PrintAdmissionRefusal::UnsupportedColor
        ))
    );
    assert_eq!(registry.admitted(), 0);
    registry
        .admit(
            &color,
            &support(true),
            &binding(),
            identity("attempt/refused"),
        )
        .unwrap();
}

#[test]
fn completed_job_cannot_be_renamed_as_a_new_attempt_or_replace_its_payload() {
    let mut registry = PrintAttemptRegistry::<1>::prepare().unwrap();
    let original = job("job/completed");
    let attempt = identity("attempt/completed");
    let provider_job = identity("provider/completed");
    registry
        .admit(&original, &support(false), &binding(), attempt.clone())
        .unwrap();
    registry
        .observe(&attempt, PrintProviderReport::Dispatched)
        .unwrap();
    registry
        .observe(
            &attempt,
            PrintProviderReport::Acknowledged(provider_job.clone()),
        )
        .unwrap();
    assert!(matches!(
        registry
            .observe(
                &attempt,
                PrintProviderReport::Completed(provider_job.clone())
            )
            .unwrap(),
        PrintLifecycle::Completed(_)
    ));
    let changed = PrintRequest::new(
        PrintAttributes::new(
            PrintColor::Color,
            3,
            PrintMedia::Letter,
            PrintSides::OneSided,
        )
        .unwrap(),
        original.document().clone(),
        original.job_id().clone(),
    )
    .unwrap();
    assert_eq!(
        registry.admit(
            &changed,
            &support(true),
            &binding(),
            identity("attempt/replaced-payload")
        ),
        Err(PrintRegistryRefusal::DuplicateJobId)
    );
    assert_eq!(
        registry.attempt(&attempt).unwrap().provider_job(),
        Some(&provider_job)
    );
    assert_eq!(registry.admitted(), 1);
}
