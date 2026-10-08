//! Revision-wide cancellation guard. Publication remains a responsibility of
//! the fixed Session orchestration after all Source witnesses are retained.
pub(crate) trait ParserSessionTargets {
    /// Cancel every ingress in this Session, including an ingress consumed in a
    /// previous step of the current unpublished revision.
    fn cancel_all(&mut self);
}
pub(crate) struct ParserSessionStage<'a, T: ParserSessionTargets> {
    targets: &'a mut T,
    armed: bool,
}
impl<'a, T: ParserSessionTargets> ParserSessionStage<'a, T> {
    pub(crate) fn new(targets: &'a mut T) -> Self {
        Self {
            targets,
            armed: true,
        }
    }
    pub(crate) fn targets(&mut self) -> &mut T {
        self.targets
    }
    /// Only the fixed Session may call this after its entire revision, Source
    /// origins and publication state are ready. A component success is insufficient.
    pub(crate) fn publication_complete(mut self) {
        self.armed = false;
    }
}
impl<T: ParserSessionTargets> Drop for ParserSessionStage<'_, T> {
    fn drop(&mut self) {
        if self.armed {
            self.targets.cancel_all();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Targets {
        calls: [u32; 3],
        cancelled: [bool; 3],
    }
    impl ParserSessionTargets for Targets {
        fn cancel_all(&mut self) {
            self.cancelled.fill(true);
        }
    }
    #[test]
    fn late_refusal_cancels_every_ingress() {
        let mut targets = Targets::default();
        {
            let mut stage = ParserSessionStage::new(&mut targets);
            stage.targets().calls[0] += 1;
            stage.targets().calls[1] += 1;
        }
        assert_eq!(targets.calls, [1, 1, 0]);
        assert_eq!(targets.cancelled, [true; 3]);
    }
    #[test]
    fn unwind_cancels_prior_consumed_ingresses() {
        let mut targets = Targets::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut stage = ParserSessionStage::new(&mut targets);
            stage.targets().calls[0] += 1;
            panic!("late target failure");
        }));
        assert!(result.is_err());
        assert_eq!(targets.cancelled, [true; 3]);
    }
    #[test]
    fn complete_publication_disarms_cancellation() {
        let mut targets = Targets::default();
        ParserSessionStage::new(&mut targets).publication_complete();
        assert_eq!(targets.cancelled, [false; 3]);
    }
}
