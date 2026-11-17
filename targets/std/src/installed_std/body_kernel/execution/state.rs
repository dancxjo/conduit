//! Retained state of the existing ordinary execution loop.
use super::*;
pub(crate) struct BodyExecutionState<'a> {
    pub(super) keys: super::super::super::keyboard_input_host::KeyboardInputHost<'a>,
    pub(super) deadlines:
        super::super::super::deadline_host::InstalledDeadlineHost<PENDING_REQUESTS>,
    pub(super) clock_quality: Option<BodyTimeQuality>,
    pub(super) kernel_failure: Option<Failure>,
    pub(super) cancelling: bool,
}
impl BodyKernel<'_> {
    pub(crate) fn execution_state<'a>(
        &mut self,
        input: Option<&'a mut dyn HostedKeyboardAdapter>,
        admitted_clock_quality: Option<BodyTimeQuality>,
    ) -> BodyExecutionState<'a> {
        let mut keys = super::super::super::keyboard_input_host::KeyboardInputHost::new(
            input,
            self.operations
                .iter()
                .map(|operation| &operation.contract_id),
        );
        if let Some(admission) = self.input_origin.take() {
            // Installation can only fail for a reused or already-active host;
            // this host is freshly constructed before its first request.
            keys.install_request_admission(admission)
                .expect("fresh Body keyboard input host");
        }
        let deadlines =
            super::super::super::deadline_host::InstalledDeadlineHost::<PENDING_REQUESTS>::new();
        let clock_quality = admitted_clock_quality;
        let kernel_failure = None;
        BodyExecutionState {
            keys,
            deadlines,
            clock_quality,
            kernel_failure,
            cancelling: false,
        }
    }
}
