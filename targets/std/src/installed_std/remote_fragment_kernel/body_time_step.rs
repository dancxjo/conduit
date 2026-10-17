use super::InstalledRemoteFragment;
use conduit_kernel::scheduler::SchedulerStatus;

impl InstalledRemoteFragment {
    pub fn step(&mut self) -> Result<SchedulerStatus, String> {
        if self.body_time_required {
            return Err(
                "BodyTime-qualified remote fragment requires clock-checked stepping".into(),
            );
        }
        self.step_kernel()
    }

    pub(crate) fn require_body_time(&mut self) {
        self.body_time_required = true;
    }

    pub(crate) fn step_body_time_admitted(&mut self) -> Result<SchedulerStatus, String> {
        if !self.body_time_required {
            return Err("remote fragment has no BodyTime admission".into());
        }
        self.step_kernel()
    }

    fn step_kernel(&mut self) -> Result<SchedulerStatus, String> {
        self.scheduler
            .step()
            .map_err(|error| format!("step remote std fragment: {error:?}"))
    }
}
