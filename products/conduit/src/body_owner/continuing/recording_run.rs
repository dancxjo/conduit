//! One exact publication through the original selected-storage Body kernel.
//! Matching publication output is not independent readback or catalog custody.
use super::*;
use conduit_core as c;
use conduit_std_host::body_execution::{BodyBorrowedForeOutputAdapter, BodyForeDeliveryRef};
use source_recording_stage::publication::SourcePublicationInput;
pub(super) struct RecordingRun {
    input: SourcePublicationInput,
    matched: bool,
}
impl RecordingRun {
    pub(super) fn new(input: SourcePublicationInput) -> Self {
        Self {
            input,
            matched: false,
        }
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
        let mut capture = PublicationCapture {
            expected: &self.input.expected_published,
            matched: false,
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
        (host, result)
    }
    pub(super) fn metadata(&self, report: Option<&BodyRunReport>) -> serde_json::Value {
        let completed = report.is_some_and(|r| {
            matches!(r.terminal, c::TerminalDisposition::Completed)
                && r.failure.is_none()
                && r.kernel_failure.is_none()
                && r.native_failure.is_none()
                && r.cleanup_failure.is_none()
        });
        serde_json::json!({"custody":"publication-output-not-readback-or-catalog-commit", "matching_publication_fore":self.matched,
            "publication_completed":completed && self.matched,"producer_retirement_authorized":false})
    }
}
struct PublicationCapture<'a> {
    expected: &'a [u8],
    matched: bool,
}
impl BodyBorrowedForeOutputAdapter for PublicationCapture<'_> {
    fn deliver(&mut self, value: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        if self.matched
            || value.front_port_id.as_str() != "published"
            || value.track != c::ConnectionTrack::Payload
            || value.value_kind.as_str() != c::RESOURCE_REFERENCE_INFO_ID
            || value.sequence != 0
            || value.bytes != self.expected
        {
            return Err(
                "Source publication returned an unexpected or repeated reference Fore".into(),
            );
        }
        self.matched = true;
        Ok(())
    }
}
