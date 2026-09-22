use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef, ValueStorage,
};
use conduit_presentation::{LayoutFrame, MAX_LAYOUT_FRAME_BYTES};

macro_rules! factory {
    ($name:ident, $implementation:ident) => {
        pub(super) static $name: BackFactory = BackFactory {
            implementation_id: conduit_std_offers::$implementation,
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
pub(super) static TEST_LAYOUT_SINK_FACTORY: BackFactory = BackFactory {
    implementation_id: "conduit-test/layout-sink-implementation@1",
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct LayoutBack {
    source: Option<ValueRef>,
    pending: Option<RequestId>,
    emitted: bool,
}
#[cfg(test)]
pub(super) struct LayoutSinkBack;
#[cfg(test)]
impl<const PORTS: usize> StepBack<PORTS> for LayoutSinkBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            io.consume(PortId(0)).expect("present test layout input");
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed test layout closure");
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}
#[cfg(test)]
impl LayoutSinkBack {}
impl LayoutBack {}

impl<const PORTS: usize> StepBack<PORTS> for LayoutBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = self.source {
            if self.emitted {
                return StepOutcome::Complete;
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send(PortId(0), value)
                .expect("ready layout source output");
            self.emitted = true;
            return StepOutcome::Progress;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return StepOutcome::Fail(step_failure(42));
            }
            let Some(output) = outcome.output else {
                return StepOutcome::Fail(step_failure(41));
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_host_completion()
                .expect("observed layout Host Call completion");
            io.send(PortId(0), output.value)
                .expect("ready transformed layout output");
            self.pending = None;
            self.emitted = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || self.emitted {
                return StepOutcome::Fail(step_failure(42));
            }
            let Ok(input) = BoundedValueRef::new(value, MAX_LAYOUT_FRAME_BYTES as u32) else {
                return StepOutcome::Fail(step_failure(40));
            };
            let request = RequestId(0);
            io.consume(PortId(0)).expect("present layout input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("layout transform Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed layout input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.emitted = true;
    }
}

const fn step_failure(detail: u16) -> conduit_kernel::Failure {
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    }
}

pub(super) fn transform_bytes(
    placement: &PlannedGear,
    input: &[u8],
) -> Result<([u8; MAX_LAYOUT_FRAME_BYTES], usize), String> {
    let frame =
        LayoutFrame::decode(input).map_err(|error| format!("decode layout frame: {error:?}"))?;
    let output = conduit_semantic_catalog::execute_layout_transform(placement, frame)?;
    Ok((output.encode(), output.encoded_len()))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: MAX_LAYOUT_FRAME_BYTES as u32,
        host_requests: usize::from(
            placement.kind_id.as_str() != conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND,
        ),
        sign_items: 64,
        maximum_value_bytes: MAX_LAYOUT_FRAME_BYTES as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    let source = if placement.kind_id.as_str() == conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND {
        let frame = conduit_semantic_catalog::execute_layout_source(placement)?;
        let encoded = frame.encode();
        Some(
            values
                .store(&encoded[..frame.encoded_len()])
                .map_err(|error| format!("store layout viewport: {error:?}"))?,
        )
    } else {
        None
    };
    Ok(InstalledBack::Layout(LayoutBack {
        source,
        pending: None,
        emitted: false,
    }))
}
fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::layout_offer_for(placement.kind_id.as_str())
        .ok_or_else(|| "unsupported layout Kind".to_string())?;
    if placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
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
fn sink_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    if placement.kind_id.as_str() != "conduit-test/layout-sink" {
        return Err("wrong layout sink Kind".into());
    }
    Ok(BackBudget {
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
) -> Result<InstalledBack, String> {
    sink_budget(placement)?;
    Ok(InstalledBack::TestLayoutSink(LayoutSinkBack))
}
