//! Cold original-Back inspection; its full report scratch ends before tables.
use super::{AssignedTerminalTransduction, PreparedBacks};
pub(super) enum ReportRefusal {
    Unavailable,
    Mismatch,
}
#[inline(never)]
pub(super) fn check_actual<B: PreparedBacks<N, P>, const N: usize, const P: usize>(
    backs: &B,
    expected: &[[Option<AssignedTerminalTransduction>; P]; N],
) -> Result<(), ReportRefusal> {
    let report = backs.terminal_transductions();
    let actual = report.as_ref().ok_or(ReportRefusal::Unavailable)?;
    // Borrow the original report in place: moving it out of Option creates
    // another complete matrix on constrained-target stacks.
    // Include inactive rows: an actual Some cannot hide behind expected None.
    if actual != expected {
        return Err(ReportRefusal::Mismatch);
    }
    Ok(())
}

/// Preserve the existing refusal ordering: all original table/layout checks
/// finish before an arbitrary factory is invoked. No inert-factory assumption.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_checked_backs<
    B,
    E,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    bytes: &[u8],
    profile: &super::b::AssignedBodyPreparationProfile<'_>,
    fore: &[super::c::AssignedForeRequirement],
    layout: &super::ResidentLayout<'_>,
    factory: impl FnOnce() -> Result<B, E>,
) -> Result<B, super::Refusal<E>> {
    check_tables::<N, C, P, R, T, H>(bytes, profile, fore, layout)
        .map_err(super::Refusal::Tables)?;
    factory().map_err(super::Refusal::Back)
}

#[inline(never)]
fn check_tables<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    bytes: &[u8],
    profile: &super::b::AssignedBodyPreparationProfile<'_>,
    fore: &[super::c::AssignedForeRequirement],
    layout: &super::ResidentLayout<'_>,
) -> Result<(), conduit_kernel::assigned::AssignedKernelRefusal> {
    // A bounded preparation-only validation pass. This scratch is retired before
    // factory/report inspection. The immutable input is decoded again by the
    // separate storage constructor; no temporary heap or new capacity is used.
    super::BoundedAssignedKernelTables::<N, C, P, R, T, H>::prepare_with_fore(
        bytes,
        profile.maxima,
        profile.requirements,
        profile.computations,
        layout,
        fore,
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{scheduler::*, *};
    use core::cell::Cell;
    struct Back;
    impl StepBack<1> for Back {
        fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
            StepOutcome::Await
        }
    }
    struct Held<'a> {
        actual: Option<[[Option<AssignedTerminalTransduction>; 1]; 2]>,
        bound: &'a Cell<bool>,
    }
    // SAFETY: immutable reports describe these test owners; bind is the only
    // transfer and is deliberately never invoked by the read-only gate.
    unsafe impl PreparedBacks<2, 1> for Held<'_> {
        type Driver = Back;
        type Calls = ();
        fn terminal_transductions(&self) -> Option<[[Option<AssignedTerminalTransduction>; 1]; 2]> {
            self.actual
        }
        fn retained_backing_bytes(&self) -> Option<usize> {
            Some(0)
        }
        fn bind(self) -> ([Back; 2], ()) {
            self.bound.set(true);
            ([Back, Back], ())
        }
    }
    #[test]
    fn cold_gate_checks_complete_original_report_without_binding() {
        let bound = Cell::new(false);
        let expected = [[None]; 2];
        let valid = Held {
            actual: Some(expected),
            bound: &bound,
        };
        assert!(check_actual(&valid, &expected).is_ok());
        assert!(!bound.get());
        let mut actual = expected;
        actual[1][0] = Some(AssignedTerminalTransduction {
            input: PortId(0),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::Consume,
            abnormal: AssignedAbnormalTransduction::NotAccepted,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        });
        let wrong = Held {
            actual: Some(actual),
            bound: &bound,
        };
        assert!(check_actual(&wrong, &expected).is_err());
        assert!(!bound.get());
        let absent = Held {
            actual: None,
            bound: &bound,
        };
        assert!(check_actual(&absent, &expected).is_err());
        assert!(!bound.get());
    }
    #[test]
    fn malformed_assignment_refuses_before_arbitrary_factory_effects() {
        let called = Cell::new(false);
        let profile = super::super::b::AssignedBodyPreparationProfile {
            maxima: super::super::c::AssignedPlanMaxima {
                encoded_bytes: 1024,
                runtime_state_bytes: 1024,
                counts: [4; 13],
            },
            requirements: super::super::c::AssignedPlanRequirements {
                host: super::super::c::AssignedIdentity::from_text("host"),
                boot: super::super::c::AssignedIdentity::from_text("boot"),
                host_calls: &[],
                resources: &[],
                remote_bindings: &[],
            },
            computations: super::super::c::AssignedComputationProfile {
                maximum_text_bytes: 0,
                maximum_total_text_bytes: 0,
                requirements: &[],
            },
            fore: Some(&[]),
        };
        let result = prepare_checked_backs::<(), (), 1, 1, 1, 1, 1, 1>(
            b"malformed",
            &profile,
            &[],
            &super::super::ResidentLayout {
                records: &[],
                counts: [0; 13],
            },
            || {
                called.set(true);
                Ok(())
            },
        );
        assert!(matches!(result, Err(super::super::Refusal::Tables(_))));
        assert!(!called.get());
    }
}
