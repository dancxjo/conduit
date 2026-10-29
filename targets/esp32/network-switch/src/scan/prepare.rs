//! All fallible checks precede the actual affine ownership transfer.
pub(super) fn take_checked<T, E>(
    slot: &mut Option<T>,
    absent: E,
    check: impl FnOnce(&T) -> Result<(), E>,
) -> Result<T, E> {
    check(slot.as_ref().ok_or(absent)?)?;
    // No callback, effect or failure occurs between this borrow and transfer.
    Ok(slot.take().expect("checked occupied owner slot"))
}
pub(super) fn budgets(now: u64, scan: u64, cleanup: u64) -> bool {
    scan > now && scan - now <= 15_000 && cleanup > scan && cleanup - scan <= 5_000
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_preparation_refusal_preserves_the_exact_affine_owner() {
        struct Owner<'a>(&'a core::cell::Cell<u32>);
        impl Drop for Owner<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let dropped = core::cell::Cell::new(0);
        let mut slot = Some(Owner(&dropped));
        let identity = slot.as_ref().unwrap().0 as *const _;
        for reason in 0..8 {
            assert!(matches!(take_checked(&mut slot,99, |_|Err(reason)), Err(r) if r==reason));
            assert_eq!(slot.as_ref().unwrap().0 as *const _, identity);
            assert_eq!(dropped.get(), 0);
        }
        let owner = take_checked(&mut slot, 99, |_| Ok(())).unwrap();
        assert!(slot.is_none());
        assert_eq!(dropped.get(), 0);
        drop(owner);
        assert_eq!(dropped.get(), 1);
        assert!(matches!(take_checked(&mut slot, 99, |_| Ok(())), Err(99)));
    }
    #[test]
    fn absolute_budgets_refuse_overflow_expiry_and_extension() {
        assert!(budgets(1, 15_001, 20_001));
        for (now, scan, cleanup) in [
            (1, 1, 2),
            (2, 1, 3),
            (0, 15_001, 20_001),
            (1, 15_001, 20_002),
            (1, 2, 2),
            (1, 2, 1),
            (u64::MAX, 0, 1),
        ] {
            assert!(!budgets(now, scan, cleanup));
        }
        assert!(budgets(u64::MAX - 2, u64::MAX - 1, u64::MAX));
    }
}
