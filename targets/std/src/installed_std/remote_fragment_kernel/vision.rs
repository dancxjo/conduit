use super::InstalledRemoteFragment;
use conduit_kernel::scheduler::HostCallRequest;
use conduit_kernel::{BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallOutcome};
use core::fmt::Write;

impl InstalledRemoteFragment {
    pub fn complete_vision_host_call(
        &mut self,
        request: HostCallRequest,
        vision: Option<&mut crate::hosted_vision::FiniteHostedVisionBase>,
        observed_at_micros: u64,
    ) -> Result<bool, String> {
        let operation = self
            .lowered
            .host_calls
            .iter()
            .find(|operation| operation.node == request.node && operation.call == request.call)
            .ok_or_else(|| "remote host request has no lowered contract identity".to_string())?;
        let contract = operation.contract_id.as_str();
        if !matches!(
            contract,
            conduit_std_offers::LOCAL_VISION_MOTION_OPERATION
                | conduit_std_offers::LOCAL_VISION_OBJECTS_OPERATION
                | conduit_std_offers::LOCAL_VISION_OCR_OPERATION
                | conduit_std_offers::LOCAL_VISION_DESCRIBE_OPERATION
                | conduit_std_offers::LOCAL_VISION_EXPERIENCE_OPERATION
                | conduit_std_offers::LOCAL_VISION_TRACK_OPERATION
        ) {
            return Ok(false);
        }
        self.vision_request_sequence = self
            .vision_request_sequence
            .checked_add(1)
            .ok_or_else(|| "remote Vision request sequence exhausted".to_string())?;
        self.vision_run_id.clear();
        write!(
            self.vision_run_id,
            "{}/node-{}/request-{}",
            self.vision_active_play_id, request.node.0, self.vision_request_sequence
        )
        .map_err(|_| "remote Vision run identity exceeded admitted storage".to_string())?;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read remote Vision host input: {error:?}"))?;
        let encoded = if contract == conduit_std_offers::LOCAL_VISION_TRACK_OPERATION {
            self.vision_tracker
                .as_mut()
                .ok_or_else(|| "remote Vision tracker was not prepared".to_string())?
                .process(
                    input,
                    observed_at_micros,
                    &self.vision_clock_basis,
                    &self.vision_run_id,
                )
                .map(Some)
                .map_err(|_| crate::hosted_vision::HostedVisionRefusal::InvalidOutput)
        } else {
            let vision =
                vision.ok_or_else(|| "remote Vision request has no admitted Base".to_string())?;
            if contract == conduit_std_offers::LOCAL_VISION_MOTION_OPERATION {
                vision
                    .execute_motion(
                        input,
                        &self.vision_run_id,
                        observed_at_micros,
                        &self.vision_clock_basis,
                    )
                    .map(Some)
            } else if contract == conduit_std_offers::LOCAL_VISION_OCR_OPERATION {
                vision
                    .execute_ocr(
                        input,
                        &self.vision_run_id,
                        observed_at_micros,
                        &self.vision_clock_basis,
                    )
                    .map(Some)
            } else if contract == conduit_std_offers::LOCAL_VISION_DESCRIBE_OPERATION {
                vision.execute_describe(
                    input,
                    &self.vision_run_id,
                    observed_at_micros,
                    &self.vision_clock_basis,
                )
            } else if contract == conduit_std_offers::LOCAL_VISION_EXPERIENCE_OPERATION {
                vision.execute_experience(input)
            } else {
                vision
                    .execute_objects(
                        input,
                        &self.vision_run_id,
                        observed_at_micros,
                        &self.vision_clock_basis,
                    )
                    .map(Some)
            }
        };
        let maximum_output_bytes = operation.binding.maximum_output_bytes;
        let outcome = match encoded {
            Ok(Some(encoded)) => {
                let value = self
                    .scheduler
                    .store_host_value(encoded)
                    .map_err(|error| format!("store remote Vision result: {error:?}"))?;
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, maximum_output_bytes)
                            .map_err(|error| format!("bound remote Vision result: {error:?}"))?,
                    ),
                    failure: None,
                }
            }
            Ok(None) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
            Err(_) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(Failure {
                    code: FailureCode::HostCallFailed,
                    detail: 1,
                }),
            },
        };
        self.complete_host_call(request, outcome)?;
        Ok(true)
    }
}
