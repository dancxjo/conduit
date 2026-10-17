use super::common::*;
use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};
use conduit_kernel::scheduler::RemoteIngressOutcome;

#[test]
fn retained_source_feedback_observes_64_generations_and_closes_without_play_allocations() {
    let artifact = artifact("pure-counter");
    let identities = artifact.artifact().identity().clone();
    let plan = artifact.artifact().definition().internal_plan.clone();
    let mut run = artifact.prepare_pure(storage()).unwrap();
    assert_eq!(&run.kernel().definition().internal_plan, &plan);
    assert_eq!(plan.source_document_id, identities.source);
    assert_eq!(plan.checked_plot_id, identities.checked);
    assert_eq!(plan.expanded_plot_id, identities.expanded);
    let (begin, input) = input(&run, 0);
    let (observed, mut output) = output(&run);
    let mut seen = 0;
    let allocations = crate::allocation::allocations(|| {
        run.start().unwrap();
        assert_eq!(
            run.admit_input(&begin, 0, &input).unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 0 }
        );
        run.close_input(&begin).unwrap();
        let mut complete = false;
        for _ in 0..4096 {
            let status = run.step().unwrap();
            if let Some(sequence) = run.output_into(&observed, &mut output).unwrap() {
                assert_eq!(sequence, seen);
                assert_eq!(count(&output.encoded), seen);
                seen += 1;
                run.complete_output(&observed, sequence).unwrap();
            }
            if status == KernelCompositeStatus::Complete {
                complete = true;
                break;
            }
        }
        assert!(complete);
        assert_eq!(seen, 64);
        assert_eq!(
            run.output_terminal_into(&observed, &mut output).unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
    });
    assert_eq!(allocations, 0);
}

#[test]
fn held_fore_output_keeps_the_same_generation_then_drains_in_order() {
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
    let retained = output.encoded.clone();
    for _ in 0..128 {
        run.step().unwrap();
    }
    assert_eq!(run.output_into(&observed, &mut output).unwrap(), Some(0));
    assert_eq!(output.encoded, retained);
    run.complete_output(&observed, 0).unwrap();
    let mut next = 1;
    let mut complete = false;
    for _ in 0..4096 {
        let status = run.step().unwrap();
        if let Some(sequence) = run.output_into(&observed, &mut output).unwrap() {
            assert_eq!(sequence, next);
            assert_eq!(count(&output.encoded), next);
            next += 1;
            run.complete_output(&observed, sequence).unwrap();
        }
        if status == KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    assert!(complete);
    assert_eq!(next, 64);
    assert_eq!(
        run.output_terminal_into(&observed, &mut output).unwrap(),
        Some(KernelCompositeTerminal::Normal)
    );
}
