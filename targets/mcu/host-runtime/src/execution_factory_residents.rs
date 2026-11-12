//! Disjoint original resident loans and exact slot inspection.
use super::*;
/// Implemented by the linked generated resident inventory's existing opaque
/// selection ticket; it must not create a different interpreter or decoder.
/// # Safety
/// `nodes` must describe exactly the initialized, exclusively borrowed owners
/// returned by `bind`; binding must be infallible and allocate no memory.
/// A Some terminal report must match the same selected slots and bound Back
/// reports exactly, remaining immutable through bind.
pub unsafe trait PreparedResidents<const E: usize> {
    type Back;
    fn nodes(&self) -> [Option<u16>; E];
    /// Same selected slots and actual Back profiles returned by bind. Inspection
    /// changes no availability; None means this inventory cannot be inspected.
    fn terminal_transductions<const P: usize>(
        &self,
    ) -> Option<[Option<[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]>; E]>
    {
        None
    }

    /// Inspect one original slot. None is uninspectable/out of bounds;
    /// Some(None) is an unselected slot, not a fabricated empty report.
    fn terminal_transductions_at<const P: usize>(
        &self,
        slot: usize,
    ) -> Option<Option<[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]>> {
        self.terminal_transductions::<P>()?.get(slot).copied()
    }

    fn retained_backing_bytes(&self) -> Option<usize>;
    fn bind(self) -> [Option<(u16, Self::Back)>; E];
}

/// One disjoint initialized resident entry, preflighted before splitting a
/// generated inventory. Splitting alone cannot mark it unavailable or Start it.
/// # Safety
/// `node` and `bind` must refer to the same original checked loan; bind is
/// allocation/effect-free, and dropping an unbound loan preserves availability.
/// A Some terminal report must exactly match that bound Back and remain
/// immutable through bind; inspection must not change availability.
pub unsafe trait PreparedResidentEntry {
    type Back;
    fn node(&self) -> u16;
    /// A truthful immutable report of this exact loan's bound Back, without
    /// transferring it or changing its initialized owner's availability.
    fn terminal_transductions<const P: usize>(
        &self,
    ) -> Option<[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]> {
        None
    }

    fn retained_backing_bytes(&self) -> Option<usize>;
    fn bind(self) -> (u16, Self::Back);
}
/// A fragment's subset of a single collectively preflighted resident inventory.
/// Different fragments can hold disjoint tickets until the one atomic admission.
pub struct PreparedResidentSubset<L, const E: usize>(pub [Option<L>; E]);
// SAFETY: each opaque entry is exclusively held once in this owned array. The
// adapter preserves the checked mapping and delegates only infallible binds.
unsafe impl<L: PreparedResidentEntry, const E: usize> PreparedResidents<E>
    for PreparedResidentSubset<L, E>
{
    type Back = L::Back;
    fn nodes(&self) -> [Option<u16>; E] {
        core::array::from_fn(|i| self.0[i].as_ref().map(L::node))
    }
    fn terminal_transductions<const P: usize>(
        &self,
    ) -> Option<[Option<[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]>; E]>
    {
        let mut reports = [None; E];
        for (slot, loan) in reports.iter_mut().zip(&self.0) {
            if let Some(loan) = loan {
                *slot = Some(loan.terminal_transductions::<P>()?);
            }
        }
        Some(reports)
    }
    fn terminal_transductions_at<const P: usize>(
        &self,
        slot: usize,
    ) -> Option<Option<[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]>> {
        match self.0.get(slot)?.as_ref() {
            Some(loan) => Some(Some(loan.terminal_transductions::<P>()?)),
            None => Some(None),
        }
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.0
            .iter()
            .flatten()
            .try_fold(0usize, |n, l| n.checked_add(l.retained_backing_bytes()?))
    }
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    fn bind(self) -> [Option<(u16, Self::Back)>; E] {
        self.0.map(|l| l.map(L::bind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    struct Loan<'a> {
        inspectable: bool,
        available: &'a Cell<bool>,
    }
    // SAFETY: this fixture's empty row is its actual bound Back's empty law;
    // pure inspection preserves the original availability until bind.
    unsafe impl PreparedResidentEntry for Loan<'_> {
        type Back = HostCallBack;
        fn node(&self) -> u16 {
            0
        }
        fn terminal_transductions<const P: usize>(
            &self,
        ) -> Option<[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]> {
            self.inspectable.then_some([None; P])
        }
        fn retained_backing_bytes(&self) -> Option<usize> {
            Some(0)
        }
        fn bind(self) -> (u16, Self::Back) {
            self.available.set(false);
            (0, HostCallBack::new(1))
        }
    }
    #[test]
    fn slot_inspection_preserves_absence_refusal_and_original_availability() {
        let available = Cell::new(true);
        let subset = PreparedResidentSubset([
            Some(Loan {
                inspectable: true,
                available: &available,
            }),
            None,
            Some(Loan {
                inspectable: false,
                available: &available,
            }),
        ]);
        let ((selected, absent, unknown, outside), cost) =
            crate::member_return_tests::return_allocation_probe::observe(|| {
                (
                    subset.terminal_transductions_at::<2>(0),
                    subset.terminal_transductions_at::<2>(1),
                    subset.terminal_transductions_at::<2>(2),
                    subset.terminal_transductions_at::<2>(3),
                )
            });
        assert_eq!(selected, Some(Some([None; 2])));
        assert_eq!(absent, Some(None));
        assert_eq!(unknown, None);
        assert_eq!(outside, None);
        assert_eq!(cost.allocation_attempts, 0);
        assert!(available.get());
        // A different uninspectable selected slot prevents a full report, but
        // cannot force this per-slot entrance to materialize/query all E rows.
        assert!(subset.terminal_transductions::<2>().is_none());
        let subset = PreparedResidentSubset([
            Some(Loan {
                inspectable: true,
                available: &available,
            }),
            None,
        ]);
        let full = subset.terminal_transductions::<2>().unwrap();
        assert_eq!(subset.terminal_transductions_at::<2>(0), Some(full[0]));
        assert_eq!(subset.terminal_transductions_at::<2>(1), Some(full[1]));
        drop(subset);
        assert!(available.get());
    }
}
