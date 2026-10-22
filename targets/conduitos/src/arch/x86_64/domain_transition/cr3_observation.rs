//! Single-consumer Root snapshot; all timestamp arithmetic is checked here.
unsafe extern "C" {
    static mut conduitos_cr3_observation: [u64; 9];
}
/// Returns entry-control, entry-write, return-control and return-write raw TSC
/// envelopes. No subtraction of overhead or calibrated-time inference.
pub(super) fn take() -> Option<[u64; 4]> {
    // The proof appliance is single-CPU. Its baseline consumes immediately after
    // enter returns, before any later protected entry. An interrupt only records
    // Source facts; it cannot enter a different domain or write this snapshot.
    let pointer = core::ptr::addr_of_mut!(conduitos_cr3_observation).cast::<u64>();
    let raw = unsafe {
        if pointer.read_volatile() != 1 {
            return None;
        }
        let raw = core::array::from_fn::<_, 8, _>(|index| pointer.add(index + 1).read_volatile());
        pointer.write_volatile(0);
        raw
    };
    durations(raw)
}
fn durations(raw: [u64; 8]) -> Option<[u64; 4]> {
    if raw.windows(2).any(|pair| pair[1] < pair[0]) {
        return None;
    }
    Some([
        raw[1].checked_sub(raw[0])?,
        raw[3].checked_sub(raw[2])?,
        raw[5].checked_sub(raw[4])?,
        raw[7].checked_sub(raw[6])?,
    ])
}
#[cfg(test)]
mod tests {
    use super::{durations, take};
    #[unsafe(no_mangle)]
    static mut conduitos_cr3_observation: [u64; 9] = [0; 9];
    #[test]
    fn raw_envelopes_remain_distinct_and_every_regression_refuses() {
        assert_eq!(
            durations([10, 12, 20, 27, 30, 33, 40, 49]),
            Some([2, 7, 3, 9])
        );
        assert_eq!(durations([0; 8]), Some([0; 4]));
        assert_eq!(
            durations([10, 12, 11, 27, 30, 33, 40, 49]),
            None,
            "regression between brackets also refuses"
        );
        let pointer = core::ptr::addr_of_mut!(conduitos_cr3_observation);
        unsafe {
            pointer.write([0, 10, 12, 20, 27, 30, 33, 40, 49]);
        }
        assert_eq!(take(), None);
        unsafe {
            pointer.write([1, 10, 12, 20, 27, 30, 33, 40, 49]);
        }
        assert_eq!(take(), Some([2, 7, 3, 9]));
        assert_eq!(take(), None, "a snapshot has exactly one consumer");
        unsafe {
            pointer.write([1, 10, 9, 20, 27, 30, 33, 40, 49]);
        }
        assert_eq!(take(), None);
        assert_eq!(take(), None, "a regressed snapshot is consumed too");
        for pair in 0..4 {
            let mut raw = [10, 12, 20, 27, 30, 33, 40, 49];
            raw[pair * 2 + 1] = raw[pair * 2] - 1;
            assert_eq!(durations(raw), None);
        }
    }
}
