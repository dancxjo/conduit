use super::*;

fn published(cycle: u32) -> [u32; 4] {
    [
        0x5678_9000,
        0x1234,
        (1 << 24) | 7,
        (2 << 24) | (3 << 16) | (32 << 10) | cycle,
    ]
}

#[test]
fn publication_during_ownership_read_defers_the_whole_event_on_both_wrap_cycles() {
    for cycle in [0, 1] {
        let new = published(cycle);
        let mut dma = [0xdead_beef, 0, 0, cycle ^ 1];
        let mut reads = 0;
        // The controller publishes after the consumer's first word load.
        // A payload-first reader would retain dead_beef, then accept new control.
        let event = read_owned_event(cycle, |index| {
            let word = dma[index];
            reads += 1;
            dma = new;
            word
        });
        assert_eq!(event, None);
        assert_eq!(reads, 1, "unowned payload must not be read");
        let event = read_owned_event(cycle, |index| dma[index]).unwrap();
        assert_eq!(event.pointer, 0x1234_5678_9000);
        assert_eq!(event.event_type, 32);
        assert_eq!(event.completion_code, 1);
        assert_eq!(event.residual, 7);
        assert_eq!((event.slot, event.endpoint), (2, 3));
    }
}

#[test]
fn unowned_slot_never_exposes_previous_lap_payload() {
    for cycle in [0, 1] {
        assert_eq!(
            read_owned_event(cycle, |index| {
                assert_eq!(index, 3, "only ownership is observable before publication");
                cycle ^ 1
            }),
            None
        );
    }
}

#[test]
fn owned_refusals_reach_the_existing_completion_validator_unchanged() {
    let mut words = published(1);
    words[2] = (6 << 24) | 3;
    words[3] = (9 << 24) | (5 << 16) | (33 << 10) | 1;
    let event = read_owned_event(1, |index| words[index]).unwrap();
    assert_eq!((event.event_type, event.completion_code), (33, 6));
    assert_eq!((event.slot, event.endpoint, event.residual), (9, 5, 3));
}
