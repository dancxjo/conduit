//! Exact publication/readback through the original selected-storage Body kernel.
//! Matching publication output is not independent readback or catalog custody.
use super::*;
use conduit_core as c;
use conduit_std_host::body_execution::{BodyBorrowedForeOutputAdapter, BodyForeDeliveryRef};
use source_recording_stage::publication::SourcePublicationInput;
/// Constructible only from this worker's matching admitted readback and clean
/// terminal report. It is volatile input custody, never a catalog/erase receipt.
pub(super) struct VerifiedSourceSnapshot {
    reference: c::BoundedResourceRef,
}
impl VerifiedSourceSnapshot {
    pub(super) fn reference(&self) -> &c::BoundedResourceRef {
        &self.reference
    }
}
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
        let mut capture = SnapshotCapture {
            expected: &self.input.expected_output,
            matched: false,
            readback: self.input.readback,
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
    pub(super) fn verified_snapshot(
        &self,
        report: Option<&BodyRunReport>,
    ) -> Option<VerifiedSourceSnapshot> {
        (self.input.readback && self.matched && report.is_some_and(clean_completed)).then(|| {
            VerifiedSourceSnapshot {
                reference: self.input.reference.clone(),
            }
        })
    }
    pub(super) fn metadata(&self, report: Option<&BodyRunReport>) -> serde_json::Value {
        let completed = report.is_some_and(clean_completed);
        if self.input.readback {
            return serde_json::json!({"custody":"identical-snapshot-readback-not-catalog-commit", "matching_content_fore":self.matched,
                "snapshot_readback_completed":completed && self.matched,"producer_retirement_authorized":false});
        }
        serde_json::json!({"custody":"publication-output-not-readback-or-catalog-commit", "matching_publication_fore":self.matched,
            "publication_completed":completed && self.matched,"producer_retirement_authorized":false})
    }
}
struct SnapshotCapture<'a> {
    expected: &'a [u8],
    matched: bool,
    readback: bool,
}
impl BodyBorrowedForeOutputAdapter for SnapshotCapture<'_> {
    fn deliver(&mut self, value: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        if self.matched
            || value.front_port_id.as_str()
                != if self.readback {
                    "content"
                } else {
                    "published"
                }
            || value.track != c::ConnectionTrack::Payload
            || value.value_kind.as_str()
                != if self.readback {
                    "text/json-utf8@1"
                } else {
                    c::RESOURCE_REFERENCE_INFO_ID
                }
            || value.sequence != 0
            || value.bytes != self.expected
        {
            return Err("Source snapshot operation returned unexpected or repeated Fore".into());
        }
        self.matched = true;
        Ok(())
    }
}

fn clean_completed(r: &BodyRunReport) -> bool {
    matches!(r.terminal, c::TerminalDisposition::Completed)
        && r.failure.is_none()
        && r.kernel_failure.is_none()
        && r.native_failure.is_none()
        && r.cleanup_failure.is_none()
}
