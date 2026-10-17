use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_process::*;
#[path = "common/print_fixture.rs"]
mod fixture;
use fixture::*;

fn attempt() -> PrintAttempt {
    PrintAttempt::prepare(
        &request(PrintMedia::A4, PrintColor::Monochrome, PrintSides::OneSided),
        &support(false),
        &binding(),
        identity("attempt/one"),
    )
    .unwrap()
}

#[test]
fn three_jobs_admit_or_refuse_against_exact_selected_back_facts() {
    let mono = request(
        PrintMedia::A4,
        PrintColor::Monochrome,
        PrintSides::DuplexLongEdge,
    );
    let color = request(PrintMedia::Letter, PrintColor::Color, PrintSides::OneSided);
    let unsupported = request(PrintMedia::A3, PrintColor::Monochrome, PrintSides::OneSided);
    assert_eq!(
        admit_print_request(&mono, &support(false), &binding()),
        Ok(())
    );
    assert_eq!(
        admit_print_request(&color, &support(false), &binding()),
        Err(PrintAdmissionRefusal::UnsupportedColor)
    );
    assert_eq!(
        admit_print_request(&color, &support(true), &binding()),
        Ok(())
    );
    assert_eq!(
        admit_print_request(&unsupported, &support(true), &binding()),
        Err(PrintAdmissionRefusal::UnsupportedMedia)
    );
    for job in [mono, color, unsupported] {
        assert_eq!(
            PrintRequest::decode(&job.clone().encode().unwrap()).unwrap(),
            job
        );
    }
}

#[test]
fn bounded_native_data_cannot_grant_a_different_document_or_authority() {
    assert!(PrintIdentity::new("".into()).is_err());
    assert!(PrintIdentity::new("é".repeat(65)).is_err());
    for copies in [0, 101] {
        assert!(PrintAttributes::new(
            PrintColor::Monochrome,
            copies,
            PrintMedia::A4,
            PrintSides::OneSided
        )
        .is_err());
    }
    let job = request(PrintMedia::A4, PrintColor::Monochrome, PrintSides::OneSided);
    for mutation in 0..5 {
        let mut current = binding();
        match mutation {
            0 => current.identity = ResourceSemanticIdentity::from_digest([9; 32]),
            1 => current.version = ResourceVersionIdentity::from_digest([9; 32]),
            2 => current.authority_contract = "foreign/authority".into(),
            3 => current.authority_grant = "".into(),
            4 => current.maximum_bytes = 2048,
            _ => unreachable!(),
        }
        assert!(matches!(
            admit_print_request(&job, &support(false), &current),
            Err(PrintAdmissionRefusal::Document(_))
        ));
    }
}

#[test]
fn delivery_and_acknowledgement_never_forge_completion() {
    let mut current = attempt();
    assert!(matches!(current.queued(), PrintLifecycle::Queued(_)));
    assert!(matches!(
        current.observe(PrintProviderReport::Dispatched).unwrap(),
        PrintLifecycle::Dispatched(_)
    ));
    assert!(matches!(
        current
            .observe(PrintProviderReport::Acknowledged(identity(
                "provider/job-1"
            )))
            .unwrap(),
        PrintLifecycle::Acknowledged(_)
    ));
    assert_eq!(current.stage(), PrintStage::Acknowledged);
    assert_eq!(
        current.observe(PrintProviderReport::Completed(identity("provider/foreign"))),
        Err(PrintProtocolRefusal::ProviderJobIdentity)
    );
    assert_eq!(current.stage(), PrintStage::Acknowledged);
    assert!(matches!(
        current
            .observe(PrintProviderReport::Completed(identity("provider/job-1")))
            .unwrap(),
        PrintLifecycle::Completed(_)
    ));
    assert_eq!(current.stage(), PrintStage::Terminal);
    assert_eq!(
        current.observe(PrintProviderReport::Dispatched),
        Err(PrintProtocolRefusal::InvalidTransition)
    );
}

#[test]
fn unknown_ack_loss_and_post_dispatch_cancellation_are_uncertain() {
    for report in [
        PrintProviderReport::AcknowledgementUnknown,
        PrintProviderReport::ProviderLost,
        PrintProviderReport::CancellationRequested,
    ] {
        let mut current = attempt();
        current.observe(PrintProviderReport::Dispatched).unwrap();
        let event = current.observe(report).unwrap();
        assert!(matches!(event, PrintLifecycle::Uncertain(_)));
        assert_eq!(
            PrintLifecycle::decode(&event.clone().encode().unwrap()).unwrap(),
            event
        );
        assert_eq!(
            current.observe(PrintProviderReport::Dispatched),
            Err(PrintProtocolRefusal::InvalidTransition)
        );
    }
    let mut current = attempt();
    current.observe(PrintProviderReport::Dispatched).unwrap();
    current
        .observe(PrintProviderReport::Acknowledged(identity(
            "provider/job-1",
        )))
        .unwrap();
    let PrintLifecycle::Uncertain(event) =
        current.observe(PrintProviderReport::ProviderLost).unwrap()
    else {
        panic!("uncertain expected");
    };
    assert_eq!(
        event.provider_job_id().as_ref(),
        Some(&identity("provider/job-1"))
    );
    assert_eq!(current.provider_job(), Some(&identity("provider/job-1")));
}

#[test]
fn pre_dispatch_refusal_and_cancellation_remain_definite() {
    for report in [
        PrintProviderReport::RejectedBeforeDispatch,
        PrintProviderReport::ProviderLost,
    ] {
        let mut current = attempt();
        assert!(matches!(
            current.observe(report).unwrap(),
            PrintLifecycle::Failed(_)
        ));
    }
    assert!(matches!(
        attempt()
            .observe(PrintProviderReport::CancellationRequested)
            .unwrap(),
        PrintLifecycle::Cancelled(_)
    ));
    let mut current = attempt();
    assert_eq!(
        current.observe(PrintProviderReport::Completed(identity("provider/job-1"))),
        Err(PrintProtocolRefusal::InvalidTransition)
    );
    assert_eq!(current.stage(), PrintStage::Queued);
}

#[test]
fn selected_back_support_uses_the_exact_native_property_profile() {
    let facts = support(true);
    let property = facts.property().unwrap();
    assert_eq!(PrintSupport::from_property(&property).unwrap(), facts);
    let wrong = request(PrintMedia::A4, PrintColor::Monochrome, PrintSides::OneSided)
        .into_structured()
        .unwrap();
    let property = StructuredConfigurationValue::new(
        wrong.value_type().profile().unwrap().value_kind().clone(),
        wrong.canonical_bytes().unwrap(),
    )
    .unwrap();
    assert!(PrintSupport::from_property(&property).is_err());
}

#[test]
fn maximum_wire_frames_fit_the_admitted_port_bound() {
    let id = identity(&"x".repeat(128));
    let operation = PrintOperation::new(id.clone(), id.clone()).unwrap();
    let request = PrintRequest::new(
        PrintAttributes::new(
            PrintColor::Color,
            100,
            PrintMedia::A3,
            PrintSides::DuplexShortEdge,
        )
        .unwrap(),
        PrintDocument::new(reference()).unwrap(),
        id.clone(),
    )
    .unwrap();
    assert!(request.encode().unwrap().len() <= PRINT_MAXIMUM_WIRE_BYTES);
    for event in [
        PrintLifecycle::queued(id.clone(), id.clone()).unwrap(),
        PrintLifecycle::dispatched(id.clone(), id.clone()).unwrap(),
        PrintLifecycle::acknowledged(operation.clone(), id.clone()).unwrap(),
        PrintLifecycle::completed(operation.clone(), id.clone()).unwrap(),
        PrintLifecycle::failed(operation.clone(), PrintFailure::ProviderReportedFailure).unwrap(),
        PrintLifecycle::cancelled(id.clone(), id.clone()).unwrap(),
        PrintLifecycle::uncertain(
            operation,
            Some(id),
            PrintUncertainty::ProviderLostAfterDispatch,
        )
        .unwrap(),
    ] {
        let bytes = event.clone().encode().unwrap();
        assert!(bytes.len() <= PRINT_MAXIMUM_WIRE_BYTES);
        assert_eq!(PrintLifecycle::decode(&bytes).unwrap(), event);
    }
}
