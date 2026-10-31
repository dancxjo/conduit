//! Finite observations of actual attempt work; never attachment or readiness authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationSubstage {
    Prepared,
    Withdrawing,
    Associating,
    WaitingAddress,
    StartingServices,
    Ready,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StationProgress {
    pub stage: StationSubstage,
    pub attempt_started_ms: Option<u64>,
    pub stage_started_ms: u64,
    pub observed_at_ms: u64,
    pub link_up: bool,
    pub address_up: bool,
    pub services_ready: bool,
    pub captured: bool,
}
impl StationProgress {
    pub const fn new() -> Self {
        Self {
            stage: StationSubstage::Prepared,
            attempt_started_ms: None,
            stage_started_ms: 0,
            observed_at_ms: 0,
            link_up: false,
            address_up: false,
            services_ready: false,
            captured: false,
        }
    }
    pub fn enter(&mut self, stage: StationSubstage, now: u64) {
        if self.attempt_started_ms.is_none() {
            self.attempt_started_ms = Some(now);
        }
        self.stage = stage;
        self.stage_started_ms = now;
        self.observed_at_ms = now;
    }
    pub fn capture(&mut self, now: u64, link_up: bool, address_up: bool) {
        if self.captured {
            return;
        }
        self.captured = true;
        self.observed_at_ms = now;
        self.link_up = link_up;
        self.address_up = address_up;
    }
}
impl Default for StationProgress {
    fn default() -> Self {
        Self::new()
    }
}
/// The observation is stored before the first effectful poll. Dropping this
/// future preserves the last stage and never changes an operation deadline.
pub async fn observed_wait<T>(
    progress: &mut StationProgress,
    stage: StationSubstage,
    now: u64,
    work: impl core::future::Future<Output = T>,
) -> T {
    progress.enter(stage, now);
    work.await
}
