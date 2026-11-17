//! Step-committed gate operations over the shared route lifetime witness.
use super::*;

impl FaceCommitGate {
    pub(crate) fn prepare(
        placement: c::PlacementId,
        call: HostCallId,
        maximum_commits: u32,
    ) -> Result<Self, FaceContributionRefusal> {
        if placement.as_str().is_empty() || maximum_commits == 0 {
            return Err(FaceContributionRefusal::Bounds);
        }
        Self::prepare_limit(placement, call, Some(maximum_commits))
    }
    pub(crate) fn prepare_resident(
        placement: c::PlacementId,
        call: HostCallId,
    ) -> Result<Self, FaceContributionRefusal> {
        Self::prepare_limit(placement, call, None)
    }
    fn prepare_limit(
        placement: c::PlacementId,
        call: HostCallId,
        maximum_commits: Option<u32>,
    ) -> Result<Self, FaceContributionRefusal> {
        if placement.as_str().is_empty() {
            return Err(FaceContributionRefusal::Bounds);
        }
        Ok(Self {
            state: Arc::new(CommitState {
                placement,
                call,
                committed: AtomicU64::new(0),
                attached: AtomicBool::new(false),
                revoked: AtomicBool::new(false),
            }),
            candidate: None,
            commits: 0,
            maximum_commits,
        })
    }
    pub(crate) fn prepared_backing_bytes(&self) -> Option<usize> {
        core::mem::size_of::<CommitState>()
            .checked_add(2 * core::mem::size_of::<usize>())?
            .checked_add(self.state.placement.0.capacity())
    }
    pub(crate) fn observation(&self) -> FaceCommitObservation {
        FaceCommitObservation(self.state.clone())
    }
    pub(crate) fn begin_step(&mut self) {
        self.candidate = None;
    }
    pub(crate) fn consume_completion<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        expected: RequestId,
    ) -> Result<(), FaceContributionRefusal> {
        if self.state.revoked.load(Ordering::Acquire) {
            return Err(FaceContributionRefusal::Revoked);
        }
        if self
            .maximum_commits
            .is_some_and(|maximum| self.commits >= maximum)
            || self.commits == u32::MAX
        {
            return Err(FaceContributionRefusal::Bounds);
        }
        let (request, outcome) = io
            .host_completion()
            .ok_or(FaceContributionRefusal::Completion)?;
        if request != expected
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
        {
            return Err(FaceContributionRefusal::Completion);
        }
        io.consume_host_completion()
            .map_err(|_| FaceContributionRefusal::Completion)?;
        self.candidate = Some(request);
        Ok(())
    }
    pub(crate) fn step_committed(&mut self) {
        if let Some(request) = self.candidate.take() {
            self.commits += 1;
            self.state
                .committed
                .store(u64::from(request.0) + 1, Ordering::Release);
        }
    }
    pub(crate) fn cancel(&mut self) {
        self.candidate = None;
        self.state.revoked.store(true, Ordering::Release);
    }
}
