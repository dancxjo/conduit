//! Installed one-shot Backs for exact Text data publication and loading.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, CanonicalValue, Failure, FailureCode, HostCallDisposition, HostCallId, PortId,
    RequestId, ValueRef,
};

pub(super) static SAVE_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::DATA_SAVE_TEXT_STD_IMPLEMENTATION,
    budget,
    prepare: prepare_save,
};

pub(super) static LOAD_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::DATA_LOAD_TEXT_STD_IMPLEMENTATION,
    budget,
    prepare: prepare_load,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DataTextOperation {
    Save,
    Load,
}

pub(super) struct DataTextBack {
    operation: DataTextOperation,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
    pending: Option<(RequestId, ValueRef)>,
    complete: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for DataTextBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            let Some((expected_request, _input)) = self.pending else {
                return fail(FailureCode::InvalidLifecycle, 1);
            };
            if request != expected_request {
                return fail(FailureCode::InvalidLifecycle, 2);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if output.admitted_bytes > self.maximum_output_bytes
                        || output.value.byte_len > self.maximum_output_bytes
                    {
                        return fail(FailureCode::HostCallFailed, 3);
                    }
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed data Text Host Call completion");
                    io.send(PortId(0), output.value)
                        .expect("ready data Text output");
                    self.pending = None;
                    self.complete = true;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Denied, None, Some(failure))
                    if failure.code == FailureCode::HostCallDenied =>
                {
                    let Some(terminal) = self.terminal(failure.detail) else {
                        return fail(FailureCode::InvalidLifecycle, 4);
                    };
                    io.consume_host_completion()
                        .expect("observed semantic data Text refusal");
                    self.pending = None;
                    self.complete = true;
                    StepOutcome::Abnormal {
                        port: PortId(0),
                        terminal,
                    }
                }
                (HostCallDisposition::Cancelled, None, None) => fail(FailureCode::Cancelled, 5),
                (HostCallDisposition::Failed, None, Some(failure)) => StepOutcome::Fail(failure),
                _ => fail(FailureCode::InvalidLifecycle, 6),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || value.byte_len > self.maximum_input_bytes {
                return fail(FailureCode::InvalidInput, 7);
            }
            let Ok(input) = BoundedValueRef::new(value, self.maximum_input_bytes) else {
                return fail(FailureCode::InvalidInput, 8);
            };
            let retained = io.take_input(PortId(0)).expect("present data Text input");
            let request = RequestId(0);
            io.request_host_call(request, HostCallId(0), input)
                .expect("admitted data Text Host Call");
            self.pending = Some((request, retained));
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed data Text input closure");
            self.complete = true;
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }

    fn retains_host_call_input(&self, request: RequestId, value: ValueRef) -> bool {
        self.pending == Some((request, value))
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.complete = true;
    }
}

impl DataTextBack {
    fn new(
        operation: DataTextOperation,
        maximum_input_bytes: u32,
        maximum_output_bytes: u32,
    ) -> Self {
        Self {
            operation,
            maximum_input_bytes,
            maximum_output_bytes,
            pending: None,
            complete: false,
        }
    }

    fn terminal(&self, detail: u16) -> Option<CanonicalValue> {
        let encoded = match self.operation {
            DataTextOperation::Save => conduit_data::DataSaveTextTerminal::decode(&[detail as u8])
                .ok()
                .filter(|_| detail <= u16::from(u8::MAX))?
                .encode(),
            DataTextOperation::Load => conduit_data::DataLoadTextTerminal::decode(&[detail as u8])
                .ok()
                .filter(|_| detail <= u16::from(u8::MAX))?
                .encode(),
        };
        CanonicalValue::new(&encoded).ok()
    }
}

fn validate(placement: &PlannedGear) -> Result<(DataTextOperation, u32, u32), String> {
    let (operation, offer) = match placement.implementation_id.as_str() {
        conduit_std_offers::DATA_SAVE_TEXT_STD_IMPLEMENTATION => (
            DataTextOperation::Save,
            conduit_std_offers::data_save_text_std_offer(),
        ),
        conduit_std_offers::DATA_LOAD_TEXT_STD_IMPLEMENTATION => (
            DataTextOperation::Load,
            conduit_std_offers::data_load_text_std_offer(),
        ),
        _ => return Err("unsupported installed data Text implementation".into()),
    };
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || !placement.authority.is_empty()
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_std_offers::DATA_TEXT_GENERATION_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || placement.resources[0].content.is_some()
    {
        return Err("planned data Text operation differs from installed realization".into());
    }
    let call = &placement.host_calls[0];
    Ok((
        operation,
        call.maximum_input_bytes,
        call.maximum_output_bytes,
    ))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (_, input, output) = validate(placement)?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: input.saturating_add(output).saturating_add(1),
        host_requests: 1,
        sign_items: 8,
        maximum_value_bytes: input.max(output),
    })
}

fn prepare_save(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (operation, input, output) = validate(placement)?;
    if operation != DataTextOperation::Save {
        return Err("data save factory received a load placement".into());
    }
    Ok(InstalledBack::DataSaveText(DataTextBack::new(
        operation, input, output,
    )))
}

fn prepare_load(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (operation, input, output) = validate(placement)?;
    if operation != DataTextOperation::Load {
        return Err("data load factory received a save placement".into());
    }
    Ok(InstalledBack::DataLoadText(DataTextBack::new(
        operation, input, output,
    )))
}

const fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
