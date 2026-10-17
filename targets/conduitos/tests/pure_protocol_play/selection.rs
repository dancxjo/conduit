use super::common::*;
use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};

#[test]
fn unmatched_selection_is_dropped_and_the_later_value_keeps_its_exact_port_sequence() {
    let mut run = prepare("pure-selected");
    let (number, first) = input(&run, 0);
    let (_, second) = input(&run, 7);
    let (selected, mut output) = output(&run);
    let allocations = crate::allocation::allocations(|| {
        run.start().unwrap();
        run.admit_input(&number, 0, &first).unwrap();
        for _ in 0..32 {
            run.step().unwrap();
        }
        assert_eq!(run.output_into(&selected, &mut output).unwrap(), None);
        run.admit_input(&number, 1, &second).unwrap();
        run.close_input(&number).unwrap();
        let mut seen = false;
        let mut complete = false;
        for _ in 0..64 {
            let status = run.step().unwrap();
            if let Some(sequence) = run.output_into(&selected, &mut output).unwrap() {
                assert!(!seen);
                seen = true;
                assert_eq!(sequence, 0);
                assert_eq!(count(&output.encoded), 7);
                run.complete_output(&selected, sequence).unwrap();
            }
            if status == KernelCompositeStatus::Complete {
                complete = true;
                break;
            }
        }
        assert!(seen && complete);
        assert_eq!(
            run.output_terminal_into(&selected, &mut output).unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
    });
    assert_eq!(allocations, 0);
}
