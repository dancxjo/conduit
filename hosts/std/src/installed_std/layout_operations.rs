use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
};
use conduit_presentation::{LayoutFrame, MAX_LAYOUT_FRAME_BYTES};

macro_rules! factory {
    ($name:ident, $implementation:ident) => {
        pub(super) static $name: InstalledFactory = InstalledFactory {
            implementation_id: conduit_std_catalog::$implementation,
            budget,
            prepare,
        };
    };
}
factory!(LAYOUT_VIEWPORT_FACTORY, LAYOUT_VIEWPORT_IMPLEMENTATION);
factory!(LAYOUT_INSET_FACTORY, LAYOUT_INSET_IMPLEMENTATION);
factory!(LAYOUT_ROW_FACTORY, LAYOUT_ROW_IMPLEMENTATION);
factory!(LAYOUT_COLUMN_FACTORY, LAYOUT_COLUMN_IMPLEMENTATION);
factory!(LAYOUT_STACK_FACTORY, LAYOUT_STACK_IMPLEMENTATION);
factory!(LAYOUT_ALIGN_FACTORY, LAYOUT_ALIGN_IMPLEMENTATION);
#[cfg(test)]
pub(super) static TEST_LAYOUT_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: "conduit-test/layout-sink-implementation@1",
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct LayoutOperation {
    source: Option<ValueRef>,
    pending: Option<RequestId>,
    emitted: bool,
}
#[cfg(test)]
pub(super) struct LayoutSinkOperation;
#[cfg(test)]
impl LayoutSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0), ..
            } => OperationAction::Await,
            OperationInput::Closed { port: PortId(0) } => OperationAction::Complete,
            _ => InstalledOperation::fail(43),
        }
    }
}
impl LayoutOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        match self.source {
            Some(value) => OperationAction::Emit {
                port: PortId(0),
                value,
            },
            None => OperationAction::Await,
        }
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && !self.emitted => {
                self.pending = Some(RequestId(0));
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, MAX_LAYOUT_FRAME_BYTES as u32) {
                        Ok(value) => value,
                        Err(_) => return InstalledOperation::fail(40),
                    },
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending == Some(RequestId(0))
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return InstalledOperation::fail(41);
                };
                self.pending = None;
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(42),
        }
    }
    pub(super) fn advance(&mut self) -> OperationAction {
        if self.source.is_some() && !self.emitted {
            self.emitted = true;
            OperationAction::Complete
        } else {
            OperationAction::Complete
        }
    }
    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.emitted = true;
    }
}

pub(super) fn transform_bytes(
    placement: &PlannedGear,
    input: &[u8],
) -> Result<([u8; MAX_LAYOUT_FRAME_BYTES], usize), String> {
    let frame =
        LayoutFrame::decode(input).map_err(|error| format!("decode layout frame: {error:?}"))?;
    let output = conduit_std_catalog::execute_layout_transform(placement, frame)?;
    Ok((output.encode(), output.encoded_len()))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: MAX_LAYOUT_FRAME_BYTES as u32,
        host_requests: usize::from(
            placement.kind_id.as_str() != conduit_std_catalog::LAYOUT_VIEWPORT_KIND,
        ),
        sign_items: 64,
        maximum_value_bytes: MAX_LAYOUT_FRAME_BYTES as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let source = if placement.kind_id.as_str() == conduit_std_catalog::LAYOUT_VIEWPORT_KIND {
        let frame = conduit_std_catalog::execute_layout_source(placement)?;
        let encoded = frame.encode();
        Some(
            values
                .store(&encoded[..frame.encoded_len()])
                .map_err(|error| format!("store layout viewport: {error:?}"))?,
        )
    } else {
        None
    };
    Ok(InstalledOperation::Layout(LayoutOperation {
        source,
        pending: None,
        emitted: false,
    }))
}
fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_catalog::layout_offer_for(placement.kind_id.as_str())
        .ok_or_else(|| "unsupported layout Kind".to_string())?;
    if placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || placement.configuration.len() != offer.startup_parameters.len()
    {
        return Err("planned layout executable identity does not match its installation".into());
    }
    Ok(())
}
#[cfg(test)]
fn sink_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    if placement.kind_id.as_str() != "conduit-test/layout-sink" {
        return Err("wrong layout sink Kind".into());
    }
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: MAX_LAYOUT_FRAME_BYTES as u32,
    })
}
#[cfg(test)]
fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    sink_budget(placement)?;
    Ok(InstalledOperation::TestLayoutSink(LayoutSinkOperation))
}
