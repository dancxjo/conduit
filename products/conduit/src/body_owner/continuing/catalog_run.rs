//! A finite current-value observer for the ordinary catalog checkpoint worker.
//! Observing State is not fresh-process recovery or producer-retirement custody.
use super::*;
use conduit_core as c;
use conduit_std_host::body_execution::{BodyBorrowedForeOutputAdapter, BodyForeDeliveryRef};
use source_recording_stage::catalog::SourceCatalogInput;
pub(super) struct CatalogRun {
    input: SourceCatalogInput,
    validator: c::PreparedStructuredValueValidator,
    matched: bool,
    own_stop: bool,
}
impl CatalogRun {
    pub(super) fn new(input: SourceCatalogInput) -> Result<Self, String> {
        let expected = c::StructuredInfoValue::from_canonical_bytes(&input.expected_current)
            .map_err(|e| format!("catalog current type: {e:?}"))?;
        let validator = c::PreparedStructuredValueValidator::new(expected.value_type(), 1024)
            .map_err(|e| format!("catalog current validator: {e:?}"))?;
        Ok(Self {
            input,
            validator,
            matched: false,
            own_stop: false,
        })
    }
    pub(super) fn execute<W: std::io::Write, T: conduit_std_host::TimerAdapter, F>(
        &mut self,
        mut host: StdHost,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> (StdHost, Result<BodyRunReport, String>)
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        let mut capture = CatalogCapture {
            expected: &self.input.expected_current,
            kind: &self.input.binding.value_kind,
            validator: &self.validator,
            control: request.control,
            count: 0,
            matched: false,
            own_stop: false,
        };
        let result = host.run_body_plan_with_selected_storage_borrowed_to_with_start(
            request,
            std::slice::from_ref(&self.input.input),
            &mut capture,
            &[],
            Some(&self.input.authority),
            output,
            timer,
            started,
        );
        self.matched = capture.matched;
        self.own_stop = capture.own_stop;
        (host, result)
    }
    pub(super) fn metadata(&self, report: Option<&BodyRunReport>) -> serde_json::Value {
        let clean = report.is_some_and(|r| {
            r.failure.is_none()
                && r.kernel_failure.is_none()
                && r.native_failure.is_none()
                && r.cleanup_failure.is_none()
                && r.fore_pending_output.is_none()
                && (matches!(r.terminal, c::TerminalDisposition::Completed)
                    || (self.own_stop
                        && matches!(
                            r.terminal,
                            c::TerminalDisposition::Cancelled {
                                reason: c::CancellationReason::OperatorRequested
                            }
                        )))
        });
        serde_json::json!({"custody":"observed-catalog-state-not-fresh-process-recovery", "matching_current_fore":self.matched,
            "own_stop_after_matching_current":self.own_stop,"catalog_checkpoint_observed":clean && self.matched,
            "producer_retirement_authorized":false})
    }
}
struct CatalogCapture<'a> {
    expected: &'a [u8],
    kind: &'a c::KindId,
    validator: &'a c::PreparedStructuredValueValidator,
    control: &'a RunControl,
    count: u64,
    matched: bool,
    own_stop: bool,
}
impl BodyBorrowedForeOutputAdapter for CatalogCapture<'_> {
    fn deliver(&mut self, value: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        if self.count >= 2
            || self.matched
            || value.front_port_id.as_str() != "current"
            || value.track != c::ConnectionTrack::Payload
            || value.value_kind != self.kind
            || value.sequence != self.count
        {
            return Err("catalog returned unexpected or excess current Fore".into());
        }
        self.validator
            .validate(value.bytes)
            .map_err(|e| format!("catalog current: {e:?}"))?;
        self.count += 1;
        if value.bytes == self.expected {
            self.matched = true;
            self.control
                .request_stop(RunControlRequestId::new("owner/catalog-current-observed")?)
                .map_err(|e| format!("catalog completion stop refused: {e:?}"))?;
            self.own_stop = true;
        }
        Ok(())
    }
}
