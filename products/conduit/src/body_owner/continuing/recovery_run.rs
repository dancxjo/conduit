//! Discover the committed catalog reference through admitted Body State.
//! No expected reference, old volatile stage, or snapshot authority is supplied.
use super::*;
use conduit_core as c;
use conduit_std_host::body_execution::{BodyBorrowedForeOutputAdapter, BodyForeDeliveryRef};
use source_recording_stage::recovery::SourceCatalogRecoveryInput;
pub(in super::super) struct RecoveredCatalogReference {
    binding: conduit_std_host::state_value::DurableStateBinding,
    play: BodyPlayIdentity,
    reference: c::BoundedResourceRef,
}
impl RecoveredCatalogReference {
    pub(in super::super) fn reference(&self) -> &c::BoundedResourceRef {
        &self.reference
    }
    pub(in super::super) fn binding(&self) -> &conduit_std_host::state_value::DurableStateBinding {
        &self.binding
    }
    pub(in super::super) fn play(&self) -> &BodyPlayIdentity {
        &self.play
    }
}
pub(super) struct RecoveryRun {
    input: SourceCatalogRecoveryInput,
    validator: c::PreparedStructuredValueValidator,
    bytes: [u8; 1024],
    length: usize,
    own_stop: bool,
}
impl RecoveryRun {
    pub(super) fn new(input: SourceCatalogRecoveryInput) -> Result<Self, String> {
        let validator = c::PreparedStructuredValueValidator::new(&input.value_type, 1024)
            .map_err(|e| format!("catalog recovery validator: {e:?}"))?;
        Ok(Self {
            input,
            validator,
            bytes: [0; 1024],
            length: 0,
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
        let mut capture = RecoveryCapture {
            kind: &self.input.binding.value_kind,
            validator: &self.validator,
            control: request.control,
            bytes: &mut self.bytes,
            length: 0,
            own_stop: false,
        };
        let result = host.run_body_plan_with_selected_storage_borrowed_to_with_start(
            request,
            &[],
            &mut capture,
            &[],
            None,
            output,
            timer,
            started,
        );
        self.length = capture.length;
        self.own_stop = capture.own_stop;
        (host, result)
    }
    fn clean(&self, report: Option<&BodyRunReport>) -> bool {
        self.length > 0
            && report.is_some_and(|r| {
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
            })
    }
    pub(super) fn recovered(
        &self,
        report: Option<&BodyRunReport>,
    ) -> Result<Option<RecoveredCatalogReference>, String> {
        if !self.clean(report) {
            return Ok(None);
        }
        let value = c::StructuredInfoValue::from_canonical_bytes(&self.bytes[..self.length])
            .map_err(|e| format!("catalog recovery current: {e:?}"))?;
        match value.shape() {
            c::StructuredInfoValueShape::Variant { tag, .. } if tag == "none" => Ok(None),
            c::StructuredInfoValueShape::Variant { tag, payload } if tag == "some" => {
                let c::StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
                    return Err("catalog reference payload is not a leaf".into());
                };
                let reference = c::BoundedResourceRef::decode(bytes)
                    .map_err(|e| format!("catalog recovered reference: {e:?}"))?;
                if reference.access_class.as_str() != "resource/snapshot@1"
                    || reference.content_profile.as_str() != "text/json-utf8@1"
                    || reference.extent.bytes == 0
                    || reference.extent.bytes > 4096
                    || reference.extent.items != Some(1)
                    || reference.lifetime.expires_at.is_some()
                {
                    return Err("recovered catalog exceeds the admitted snapshot profile".into());
                }
                Ok(Some(RecoveredCatalogReference {
                    binding: self.input.binding.clone(),
                    play: report.expect("clean report").play.clone(),
                    reference,
                }))
            }
            _ => Err("catalog recovery current is not an optional reference".into()),
        }
    }
    pub(super) fn metadata(&self, report: Option<&BodyRunReport>) -> serde_json::Value {
        let result = self.recovered(report);
        serde_json::json!({"custody":"recovered-state-reference-not-catalog-content-or-recording-commit",
            "state_current_observed":self.clean(report),"catalog_reference_recovered":matches!(&result,Ok(Some(_))),
            "recovery_refusal":result.err(),"producer_retirement_authorized":false})
    }
}
struct RecoveryCapture<'a> {
    kind: &'a c::KindId,
    validator: &'a c::PreparedStructuredValueValidator,
    control: &'a RunControl,
    bytes: &'a mut [u8; 1024],
    length: usize,
    own_stop: bool,
}
impl BodyBorrowedForeOutputAdapter for RecoveryCapture<'_> {
    fn deliver(&mut self, value: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        if self.length != 0
            || value.front_port_id.as_str() != "current"
            || value.track != c::ConnectionTrack::Payload
            || value.value_kind != self.kind
            || value.sequence != 0
            || value.bytes.is_empty()
            || value.bytes.len() > self.bytes.len()
        {
            return Err("catalog recovery returned unexpected or excess current Fore".into());
        }
        self.validator
            .validate(value.bytes)
            .map_err(|e| format!("catalog recovery current: {e:?}"))?;
        self.bytes[..value.bytes.len()].copy_from_slice(value.bytes);
        self.length = value.bytes.len();
        self.control
            .request_stop(RunControlRequestId::new(
                "owner/catalog-recovery-current-observed",
            )?)
            .map_err(|e| format!("catalog recovery stop refused: {e:?}"))?;
        self.own_stop = true;
        Ok(())
    }
}
