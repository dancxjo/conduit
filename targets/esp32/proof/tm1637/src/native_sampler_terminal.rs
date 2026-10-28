//! Disposition-specific Sign inspection below actual consuming retirement.
//! This predicate cannot replace returned native providers or a settled scope.
use conduit_kernel::KernelEventKind as Kind;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Disposition {
    Completed,
    CapacityInterrupted,
}
pub(crate) fn inspect<const N: usize>(
    events: impl IntoIterator<Item = (u16, Kind)>,
    disposition: Disposition,
) -> Result<(), &'static str> {
    let mut completed = [false; N];
    let mut cancelled = false;
    for (node, kind) in events {
        match kind {
            Kind::BackCompleted => {
                let slot = completed
                    .get_mut(usize::from(node))
                    .ok_or("renewal-terminal-node")?;
                if *slot {
                    return Err("renewal-terminal-duplicate");
                }
                *slot = true;
            }
            Kind::RunCancelled if disposition == Disposition::CapacityInterrupted && !cancelled => {
                cancelled = true
            }
            Kind::RunCancelled
            | Kind::StepFuelExceeded
            | Kind::BackFailed
            | Kind::SemanticAbnormal => return Err("renewal-terminal-disposition"),
            _ => {}
        }
    }
    // Cancellation must be an actual retained kernel Sign, never an inferred
    // status or fabricated completion count. A naturally drained race is valid
    // only when every original Back has its ordinary completion Sign.
    if completed.iter().all(|complete| *complete)
        || (disposition == Disposition::CapacityInterrupted && cancelled)
    {
        Ok(())
    } else {
        Err("renewal-terminal-signs")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_prefix_is_distinct_from_normal_completion() {
        let interrupted = [(0, Kind::BackCompleted), (0, Kind::RunCancelled)];
        assert!(inspect::<2>(interrupted, Disposition::Completed).is_err());
        assert!(inspect::<2>(interrupted, Disposition::CapacityInterrupted).is_ok());
        assert!(
            inspect::<2>([(0, Kind::BackCompleted)], Disposition::CapacityInterrupted).is_err()
        );
        assert!(inspect::<2>(
            [(0, Kind::BackCompleted), (1, Kind::BackCompleted)],
            Disposition::Completed
        )
        .is_ok());
    }
    #[test]
    fn cancellation_never_overrides_failure_or_duplicate_terminal_signs() {
        for bad in [
            Kind::BackFailed,
            Kind::StepFuelExceeded,
            Kind::SemanticAbnormal,
            Kind::RunCancelled,
        ] {
            assert!(inspect::<2>(
                [(0, Kind::RunCancelled), (0, bad)],
                Disposition::CapacityInterrupted
            )
            .is_err());
        }
        assert!(inspect::<2>(
            [
                (0, Kind::BackCompleted),
                (0, Kind::BackCompleted),
                (0, Kind::RunCancelled)
            ],
            Disposition::CapacityInterrupted
        )
        .is_err());
    }
}
