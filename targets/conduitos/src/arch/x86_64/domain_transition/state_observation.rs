//! Fixed Root register/IRQ envelopes, separate from CR3 and trap costs.
unsafe extern "C" {
    static mut conduitos_state_observation: [u64; 15];
}
/// Four register envelopes then IRQ count/total/min/max/empty-control-total.
/// Zero IRQs have no measured minimum/maximum; serial realization uses null.
pub(super) fn take() -> Option<[u64; 9]> {
    let pointer = core::ptr::addr_of_mut!(conduitos_state_observation).cast::<u64>();
    // Same single-CPU immediate consumer contract as the CR3 snapshot. Root-CS
    // interrupts cannot publish aggregate records. Assembly arms each entry.
    let raw = unsafe {
        if pointer.read_volatile() != 1 {
            return None;
        }
        let raw = core::array::from_fn::<_, 14, _>(|index| pointer.add(index + 1).read_volatile());
        pointer.write_volatile(0);
        raw
    };
    validate(raw)
}
fn validate(raw: [u64; 14]) -> Option<[u64; 9]> {
    if raw[..8].windows(2).any(|pair| pair[1] < pair[0]) || raw[12] != 0 {
        return None;
    }
    let [count, total, min, max, _, control] = <[u64; 6]>::try_from(&raw[8..]).ok()?;
    if count == 0 {
        if total != 0 || min != u64::MAX || max != 0 || control != 0 {
            return None;
        }
    } else if min > max
        || u128::from(min) * u128::from(count) > u128::from(total)
        || u128::from(max) * u128::from(count) < u128::from(total)
    {
        return None;
    }
    Some([
        raw[1].checked_sub(raw[0])?,
        raw[3].checked_sub(raw[2])?,
        raw[5].checked_sub(raw[4])?,
        raw[7].checked_sub(raw[6])?,
        count,
        total,
        if count == 0 { 0 } else { min },
        max,
        control,
    ])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[unsafe(no_mangle)]
    static mut conduitos_state_observation: [u64; 15] = [0; 15];
    #[test]
    fn consumes_once_and_refuses_regression_bad_overflow_and_inconsistent_aggregates() {
        let raw = [10, 12, 20, 27, 30, 33, 40, 49, 0, 0, u64::MAX, 0, 0, 0];
        assert_eq!(validate(raw), Some([2, 7, 3, 9, 0, 0, 0, 0, 0]));
        let mut irq = raw;
        irq[8..].copy_from_slice(&[2, 10, 4, 6, 0, 3]);
        assert_eq!(validate(irq), Some([2, 7, 3, 9, 2, 10, 4, 6, 3]));
        for (index, value) in [(1, 9), (2, 11), (12, 1), (9, 7), (10, 7), (11, 3)] {
            let mut changed = irq;
            changed[index] = value;
            assert_eq!(validate(changed), None, "{index}");
        }
        for (index, value) in [(9, 1), (10, 0), (11, 1), (13, 1)] {
            let mut changed = raw;
            changed[index] = value;
            assert_eq!(validate(changed), None, "empty {index}");
        }
        let pointer = core::ptr::addr_of_mut!(conduitos_state_observation);
        let mut full = [0; 15];
        full[1..].copy_from_slice(&irq);
        unsafe {
            pointer.write(full);
        }
        assert_eq!(take(), None);
        full[0] = 1;
        unsafe {
            pointer.write(full);
        }
        assert_eq!(take(), Some([2, 7, 3, 9, 2, 10, 4, 6, 3]));
        assert_eq!(take(), None);
        full[13] = 1;
        unsafe {
            pointer.write(full);
        }
        assert_eq!(take(), None);
        assert_eq!(take(), None);
    }
}
