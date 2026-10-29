//! Reviewed ESP32 ROM/Zbit family timing envelope, not execution authority.
//! JEDEC 5e4016 does not distinguish B/D variants: use the larger published
//! values, including D's extended-temperature table, not the observed 6ms erase.
//!
//! ZB25VQ32B table 8.6: sector erase 400ms, page program 2ms, status write 100ms.
//! https://www.zbitsemi.com/upload/file/20201010/20201010174021_57537.pdf
//! ZB25VQ32D_DTR table 8.6b: sector erase 550ms, page program 5ms.
//! https://www.zbitsemi.com/upload/file/20220727/20220727140527_78598.pdf
//! Marker commits perform one sector erase, at most nine <=64-byte programs
//! (including obsolete-commit invalidation and final commit), and readback.
//! Budget 100ms for unlock/status, 550ms erase, 45ms programming and 200ms
//! ROM/readback/CPU allowance, rounded to 1000ms. Erase/readback rounds 750ms
//! to 800ms. Verification reads a bounded 4KiB, with a 100ms allowance.
//! Blue qualification 2026-10-28: 32 erases <=5998us, six marker writes <=993us,
//! no reported failures, ROM descriptor geometry matching the predicate below.
//! These bounds assume supported electrical/temperature operation; they do not
//! promise recovery from a stuck ROM routine or an out-of-spec physical device.
use crate::native_observed_reclaim_job::Operation;
/// Immutable profile evidence. Only the actual ROM-reading parent may select it;
/// received descriptors cannot construct this token. It grants no flash access.
#[derive(Clone, Copy)]
pub(crate) struct ReviewedTiming(());
pub(super) fn review(
    device_id: u32,
    capacity: u32,
    block: u32,
    sector: u32,
    page: u32,
    status_mask: u32,
) -> Option<ReviewedTiming> {
    (device_id == 0x5e4016
        && capacity == 0x400000
        && block == 65536
        && sector == 4096
        && page == 256
        && status_mask == 65535)
        .then_some(ReviewedTiming(()))
}
pub(crate) const MAXIMUM_BUDGET_MS: u32 = 1000;
impl ReviewedTiming {
    pub(crate) const fn budget_ms(self, operation: Operation) -> u32 {
        match operation {
            Operation::CommitMarker => 1000,
            Operation::EraseSector => 800,
            Operation::VerifySector => 100,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_or_mismatched_geometry_has_no_reviewed_timing() {
        for descriptor in [
            (0xef4016, 0x400000, 65536, 4096, 256, 65535),
            (0x5e4016, 0x800000, 65536, 4096, 256, 65535),
            (0x5e4016, 0x400000, 32768, 4096, 256, 65535),
            (0x5e4016, 0x400000, 65536, 8192, 256, 65535),
            (0x5e4016, 0x400000, 65536, 4096, 512, 65535),
            (0x5e4016, 0x400000, 65536, 4096, 256, 0),
        ] {
            assert!(review(
                descriptor.0,
                descriptor.1,
                descriptor.2,
                descriptor.3,
                descriptor.4,
                descriptor.5
            )
            .is_none());
        }
        let profile = review(0x5e4016, 0x400000, 65536, 4096, 256, 65535).unwrap();
        for operation in [
            Operation::CommitMarker,
            Operation::EraseSector,
            Operation::VerifySector,
        ] {
            assert!(
                profile.budget_ms(operation) > 0
                    && profile.budget_ms(operation) <= MAXIMUM_BUDGET_MS
            );
        }
        assert!(profile.budget_ms(Operation::CommitMarker) > 550 + 100 + 9 * 5 + 200);
    }
    #[test]
    fn reviewed_budgets_still_require_enough_actual_wait_slack() {
        let profile = review(0x5e4016, 0x400000, 65536, 4096, 256, 65535).unwrap();
        let gate = crate::native_flash_window::Gate::new(60_000, 60, MAXIMUM_BUDGET_MS).unwrap();
        let wait = gate.begin_wait(0, 1000).unwrap();
        assert!(matches!(
            wait.try_flash(1, profile.budget_ms(Operation::CommitMarker)),
            Err(crate::native_flash_window::Refusal::Deadline)
        ));
        let permit = wait
            .try_flash(1, profile.budget_ms(Operation::EraseSector))
            .unwrap();
        drop(permit);
        drop(wait);
        let wait = gate.begin_wait(1000, 2500).unwrap();
        assert!(wait
            .try_flash(1001, profile.budget_ms(Operation::CommitMarker))
            .is_ok());
    }
}
