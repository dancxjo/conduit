use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
};
use conduit_presentation::{PresentationComposition, MAX_PRESENTATION_COMPOSITION_BYTES};

macro_rules! factory {
    ($name:ident, $implementation:ident) => {
        pub(super) static $name: InstalledFactory = InstalledFactory {
            implementation_id: conduit_std_catalog::$implementation,
            budget,
            prepare,
        };
    };
}
factory!(PRESENTATION_ICON_FACTORY, PRESENTATION_ICON_IMPLEMENTATION);
factory!(
    PRESENTATION_FRAME_FACTORY,
    PRESENTATION_FRAME_IMPLEMENTATION
);
factory!(
    PRESENTATION_BADGE_FACTORY,
    PRESENTATION_BADGE_IMPLEMENTATION
);
#[cfg(test)]
pub(super) static TEST_PRESENTATION_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: "conduit.test/presentation-sink-implementation@1",
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct PresentationCompositionOperation {
    source: Option<ValueRef>,
    pending: bool,
    emitted: bool,
}

#[cfg(test)]
pub(super) struct PresentationSinkOperation;

#[cfg(test)]
impl PresentationSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0), ..
            } => OperationAction::Await,
            OperationInput::Closed { port: PortId(0) } => OperationAction::Complete,
            _ => InstalledOperation::fail(47),
        }
    }
}

impl PresentationCompositionOperation {
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
            } if !self.pending && !self.emitted => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(
                        value,
                        MAX_PRESENTATION_COMPOSITION_BYTES as u32,
                    ) {
                        Ok(value) => value,
                        Err(_) => return InstalledOperation::fail(44),
                    },
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return InstalledOperation::fail(45);
                };
                self.pending = false;
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(46),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.source.is_some() && !self.emitted {
            self.emitted = true;
        }
        OperationAction::Complete
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
        self.emitted = true;
    }
}

pub(super) fn transform_bytes(
    placement: &PlannedGear,
    input: &[u8],
) -> Result<([u8; MAX_PRESENTATION_COMPOSITION_BYTES], usize), String> {
    let value = PresentationComposition::decode(input)
        .map_err(|error| format!("decode presentation composition: {error:?}"))?;
    let output = match placement.kind_id.as_str() {
        conduit_std_catalog::PRESENTATION_FRAME_KIND => value.frame(
            text_config(placement, conduit_std_catalog::ROLE_KEY)?,
            text_config(placement, conduit_std_catalog::ACCESSIBILITY_NAME_KEY)?,
        ),
        conduit_std_catalog::PRESENTATION_BADGE_KIND => value.badge(
            text_config(placement, conduit_std_catalog::STATE_KEY)?,
            text_config(placement, conduit_std_catalog::ACCESSIBILITY_NAME_KEY)?,
        ),
        _ => return Err("unsupported presentation composition transform".into()),
    }
    .map_err(|error| format!("presentation composition refused: {error:?}"))?;
    Ok((output.encode(), output.encoded_len()))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: MAX_PRESENTATION_COMPOSITION_BYTES as u32,
        host_requests: usize::from(
            placement.kind_id.as_str() != conduit_std_catalog::PRESENTATION_ICON_KIND,
        ),
        sign_items: 32,
        maximum_value_bytes: MAX_PRESENTATION_COMPOSITION_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let source = if placement.kind_id.as_str() == conduit_std_catalog::PRESENTATION_ICON_KIND {
        let value = PresentationComposition::icon(
            text_config(placement, conduit_std_catalog::ICON_KEY)?,
            text_config(placement, conduit_std_catalog::ACCESSIBILITY_NAME_KEY)?,
        )
        .map_err(|error| format!("presentation icon refused: {error:?}"))?;
        let bytes = value.encode();
        Some(
            values
                .store(&bytes[..value.encoded_len()])
                .map_err(|error| format!("store presentation composition: {error:?}"))?,
        )
    } else {
        None
    };
    Ok(InstalledOperation::PresentationComposition(
        PresentationCompositionOperation {
            source,
            pending: false,
            emitted: false,
        },
    ))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_catalog::presentation_composition_offer_for(placement.kind_id.as_str())
        .ok_or_else(|| "unsupported presentation composition Kind".to_string())?;
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
        return Err(
            "planned presentation composition identity does not match its installation".into(),
        );
    }
    Ok(())
}

fn text_config<'a>(placement: &'a PlannedGear, key: &str) -> Result<&'a str, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::Text(value)) if found == key => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(|| format!("presentation configuration '{key}' is missing"))
}

#[cfg(test)]
fn sink_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    if placement.kind_id.as_str() != "conduit.test/presentation-sink" {
        return Err("wrong presentation sink Kind".into());
    }
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: MAX_PRESENTATION_COMPOSITION_BYTES as u32,
    })
}

#[cfg(test)]
fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    sink_budget(placement)?;
    Ok(InstalledOperation::TestPresentationSink(
        PresentationSinkOperation,
    ))
}
