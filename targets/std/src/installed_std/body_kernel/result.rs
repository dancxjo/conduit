//! Terminal facts moved out of the one ordinary kernel; no new execution.
use super::*;
pub(crate) struct BodyKernelResult {
    pub startup_chimes: Vec<crate::body_execution::StartupChimeCallReceipt>,
    pub terminal: TerminalDisposition,
    pub failure: Option<String>,
    pub kernel_failure: Option<Failure>,
    pub cleanup_failure: Option<String>,
    pub partitions: Vec<KernelIdentityMap>,
    pub requests: Vec<HostCallRequest>,
    pub events: Vec<KernelEvent>,
    pub scan_child_signs:
        Result<Vec<conduit_composite::ScanChildSignReceipt>, conduit_composite::BoundedScanError>,
    pub scan_cancellation_failed: bool,
    pub scan_output_completion_failed: bool,
    pub each_child_signs: Result<
        Vec<conduit_composite::ScanChildSignReceipt>,
        conduit_composite::BoundedActivationError,
    >,
    pub each_cancellation_failed: bool,
    pub each_output_completion_failed: bool,
    pub fore_deliveries: Vec<ExternalForeDelivery>,
    pub fore_pending_output: Option<crate::body_execution::BodyForePendingOutput>,
    pub clock_observations: Vec<ObservedKernelEvent>,
    pub clock_quality: Option<BodyTimeQuality>,
    pub clock_execution_bounds: Option<(
        conduit_core::MonotonicDuration,
        conduit_core::MonotonicDuration,
    )>,
}
