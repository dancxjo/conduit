use super::*;

fn encoded(trace: &Trace) -> [u8; ENCODED_BYTES] {
    let mut output = [0; ENCODED_BYTES];
    assert_eq!(trace.encode_into(&mut output), Some(ENCODED_BYTES));
    output
}
fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

#[test]
fn first_write_is_retained_and_repeats_do_not_overwrite() {
    let mut trace = Trace::new();
    assert!(trace.retain_arena(1234));
    assert!(trace.record(24, 12, 34, 56, Some(78)));
    let first = encoded(&trace);
    assert!(!trace.retain_arena(4321));
    assert!(!trace.record(24, 99, 99, 99, None));
    assert_eq!(encoded(&trace), first);
    assert_eq!(word(&first, 8), 1 << 23);
    assert_eq!(word(&first, 12), 1234);
    let offset = 16 + 23 * 20;
    for (index, expected) in [24, 12, 34, 56, 78].into_iter().enumerate() {
        assert_eq!(word(&first, offset + index * 4), expected);
    }
}

#[test]
fn invalid_phase_and_unrepresentable_mark_leave_trace_unchanged() {
    let mut trace = Trace::new();
    let empty = encoded(&trace);
    for phase in [0, 25, usize::MAX] {
        assert!(!trace.record(phase, 1, 2, 3, None));
    }
    assert!(!trace.record(1, 1, 2, 3, Some(u32::MAX as usize)));
    assert!(!trace.retain_arena(u32::MAX as usize));
    assert_eq!(encoded(&trace), empty);
    if usize::BITS > 32 {
        assert!(!trace.record(1, usize::MAX, 2, 3, None));
        assert!(!trace.record(1, 1, usize::MAX, 3, None));
        assert!(!trace.record(1, 1, 2, usize::MAX, None));
        assert!(!trace.retain_arena(usize::MAX));
        assert_eq!(encoded(&trace), empty);
    }
}

#[test]
fn encoding_capacity_refuses_before_writes_and_preserves_tail() {
    let trace = Trace::new();
    let mut short = [0x55; ENCODED_BYTES - 1];
    assert_eq!(trace.encode_into(&mut short), None);
    assert_eq!(short, [0x55; ENCODED_BYTES - 1]);
    assert_eq!(trace.encode_into(&mut []), None);
    let mut long = [0x55; ENCODED_BYTES + 7];
    assert_eq!(trace.encode_into(&mut long), Some(ENCODED_BYTES));
    assert_eq!(&long[ENCODED_BYTES..], &[0x55; 7]);
}

#[test]
fn fixed_layout_and_all_twenty_four_slots_are_preserved() {
    assert_eq!(core::mem::size_of::<Record>(), 20);
    assert_eq!(core::mem::size_of::<Trace>(), 488);
    assert_eq!(ENCODED_BYTES, 496);
    let mut trace = Trace::new();
    for phase in 1..=24 {
        assert!(trace.record(phase, phase, phase + 1, phase + 2, None));
    }
    let bytes = encoded(&trace);
    assert_eq!(&bytes[..4], b"CPTR");
    assert_eq!(word(&bytes, 4), 1);
    assert_eq!(word(&bytes, 8), 0x00ff_ffff);
    assert_eq!(word(&bytes, 12), UNAVAILABLE);
    for phase in 1..=24 {
        let offset = 16 + (phase - 1) * 20;
        assert_eq!(word(&bytes, offset), phase as u32);
        assert_eq!(word(&bytes, offset + 16), UNAVAILABLE);
    }
}
