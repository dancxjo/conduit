use super::common::*;
use conduit_composite::{KernelCompositeError, KernelCompositeStatus};
use conduit_kernel::{FailureCode, KernelEventKind};
use conduitos::{
    protocol_host_calls::ProtocolCallRefusal,
    usb_base::{control_proof_plan::ControlProofSubject, device_probe_proof_plan},
};

#[test]
fn an_exact_usb_control_plan_cannot_enter_the_pure_profile() {
    let subject = ControlProofSubject {
        host_id: "fixture/physical-host",
        boot_id: "fixture/physical-boot",
        controller_base_id: "fixture/controller",
        device_instance_id: "fixture/device",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
    };
    let artifact = device_probe_proof_plan::prepare(&subject).unwrap();
    assert!(
        artifact.artifact().definition().internal_plan.fragments[0]
            .placements
            .iter()
            .any(|gear| gear.base.is_some())
    );
    assert!(matches!(
        artifact.prepare_pure(storage()),
        Err(ProtocolCallRefusal::Unsupported)
    ));
}

#[test]
fn unstarted_cancellation_cannot_be_followed_by_start() {
    let mut run = prepare("pure-counter");
    let allocations = crate::allocation::allocations(|| {
        run.cancel().unwrap();
        assert_eq!(run.step().unwrap(), KernelCompositeStatus::Cancelled);
        assert!(matches!(
            run.start(),
            Err(ProtocolCallRefusal::Kernel(
                KernelCompositeError::InvalidLifecycle
            ))
        ));
    });
    assert_eq!(allocations, 0);
}

#[test]
fn arithmetic_failure_completes_one_host_call_and_requires_explicit_cancellation() {
    let mut run = prepare("pure-overflow");
    let (number, input) = input(&run, u64::MAX);
    let (advanced, mut output) = output(&run);
    run.start().unwrap();
    run.admit_input(&number, 0, &input).unwrap();
    run.close_input(&number).unwrap();
    let mut failed = false;
    for _ in 0..64 {
        if let Err(error) = run.step() {
            assert!(matches!(error, ProtocolCallRefusal::Expression(_)));
            assert_eq!(error.failure().code, FailureCode::HostCallFailed);
            assert_eq!(error.failure().detail, 1027);
            failed = true;
            break;
        }
        assert_eq!(run.output_into(&advanced, &mut output).unwrap(), None);
    }
    assert!(failed);
    assert!(matches!(run.step(), Err(ProtocolCallRefusal::Kernel(_))));
    run.cancel().unwrap();
    assert_eq!(run.step().unwrap(), KernelCompositeStatus::Cancelled);
    let signs = run.kernel().signs();
    for kind in [
        KernelEventKind::HostCallRequested,
        KernelEventKind::HostCallCompleted,
        KernelEventKind::BackFailed,
    ] {
        assert_eq!(
            signs
                .values()
                .flatten()
                .filter(|event| event.kind == kind)
                .count(),
            1
        );
    }
    assert!(matches!(
        run.output_into(&advanced, &mut output),
        Err(ProtocolCallRefusal::Kernel(
            KernelCompositeError::InvalidLifecycle
        ))
    ));
    assert!(matches!(
        run.output_terminal_into(&advanced, &mut output),
        Err(ProtocolCallRefusal::Kernel(
            KernelCompositeError::InvalidLifecycle
        ))
    ));
}

#[test]
fn cancellation_under_output_pressure_revokes_execution_without_allocating() {
    let mut run = prepare("pure-counter");
    let (begin, input) = input(&run, 0);
    let (observed, mut output) = output(&run);
    run.start().unwrap();
    run.admit_input(&begin, 0, &input).unwrap();
    run.close_input(&begin).unwrap();
    for _ in 0..128 {
        run.step().unwrap();
    }
    assert_eq!(run.output_into(&observed, &mut output).unwrap(), Some(0));
    let allocations = crate::allocation::allocations(|| {
        run.cancel().unwrap();
        assert_eq!(run.step().unwrap(), KernelCompositeStatus::Cancelled);
        assert!(matches!(
            run.output_into(&observed, &mut output),
            Err(ProtocolCallRefusal::Kernel(
                KernelCompositeError::InvalidLifecycle
            ))
        ));
        assert!(matches!(
            run.complete_output(&observed, 0),
            Err(ProtocolCallRefusal::Kernel(
                KernelCompositeError::InvalidLifecycle
            ))
        ));
        assert!(matches!(
            run.admit_input(&begin, 1, &input),
            Err(ProtocolCallRefusal::Kernel(
                KernelCompositeError::InvalidLifecycle
            ))
        ));
    });
    assert_eq!(allocations, 0);
    assert!(
        run.kernel()
            .signs()
            .values()
            .flatten()
            .any(|event| event.kind == KernelEventKind::RunCancelled)
    );
}
