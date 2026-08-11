use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
};
use conduit_presentation::{
    GraphicsScene, PresentationComposition, MAX_GRAPHICS_SCENE_BYTES,
    MAX_PRESENTATION_COMPOSITION_BYTES,
};

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
factory!(GRAPHICS_RECT_FACTORY, GRAPHICS_RECT_IMPLEMENTATION);
factory!(GRAPHICS_TEXT_FACTORY, GRAPHICS_TEXT_IMPLEMENTATION);
factory!(GRAPHICS_ICON_FACTORY, GRAPHICS_ICON_IMPLEMENTATION);
pub(super) static GRAPHICS_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::GRAPHICS_PRESENTATION_IMPLEMENTATION,
    budget: graphics_presentation_budget,
    prepare: prepare_graphics_presentation,
};
#[cfg(test)]
pub(super) static TEST_PRESENTATION_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: "conduit.test/presentation-sink-implementation@1",
    budget: sink_budget,
    prepare: prepare_sink,
};
#[cfg(test)]
pub(super) static TEST_GRAPHICS_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: "conduit.test/graphics-sink-implementation@1",
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct PresentationCompositionOperation {
    source: Option<ValueRef>,
    pending: bool,
    emitted: bool,
}

pub(super) struct GraphicsPresentationOperation {
    pending: bool,
    presented: bool,
}

impl GraphicsPresentationOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.presented => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, MAX_GRAPHICS_SCENE_BYTES as u32) {
                        Ok(value) => value,
                        Err(_) => return InstalledOperation::fail(48),
                    },
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                self.pending = false;
                self.presented = true;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(48),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
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
    let output = conduit_std_catalog::execute_presentation_transform(placement, value)?;
    Ok((output.encode(), output.encoded_len()))
}

pub(super) fn transform_graphics_bytes(
    placement: &PlannedGear,
    input: &[u8],
) -> Result<([u8; MAX_GRAPHICS_SCENE_BYTES], usize), String> {
    let (composition, scene) =
        if placement.kind_id.as_str() == conduit_std_catalog::GRAPHICS_RECT_KIND {
            let composition = PresentationComposition::decode(input)
                .map_err(|error| format!("decode presentation composition: {error:?}"))?;
            (Some(composition), None)
        } else {
            (
                None,
                Some(
                    GraphicsScene::decode(input)
                        .map_err(|error| format!("decode graphics scene: {error:?}"))?,
                ),
            )
        };
    let scene = conduit_std_catalog::execute_graphics_transform(placement, composition, scene)?;
    Ok((scene.encode(), scene.encoded_len()))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: MAX_PRESENTATION_COMPOSITION_BYTES.max(MAX_GRAPHICS_SCENE_BYTES) as u32,
        host_requests: usize::from(
            placement.kind_id.as_str() != conduit_std_catalog::PRESENTATION_ICON_KIND,
        ),
        sign_items: 32,
        maximum_value_bytes: MAX_PRESENTATION_COMPOSITION_BYTES.max(MAX_GRAPHICS_SCENE_BYTES)
            as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let source = if placement.kind_id.as_str() == conduit_std_catalog::PRESENTATION_ICON_KIND {
        let value = conduit_std_catalog::execute_presentation_source(placement)?;
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
        .or_else(|| conduit_std_catalog::graphics_offer_for(placement.kind_id.as_str()))
        .ok_or_else(|| "unsupported presentation or graphics Kind".to_string())?;
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

fn graphics_presentation_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_graphics_presentation(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: MAX_GRAPHICS_SCENE_BYTES as u32,
    })
}

fn prepare_graphics_presentation(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_graphics_presentation(placement)?;
    Ok(InstalledOperation::GraphicsPresentation(
        GraphicsPresentationOperation {
            pending: false,
            presented: false,
        },
    ))
}

fn validate_graphics_presentation(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_catalog::graphics_presentation_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str() != conduit_core::PRESENTATION_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || !placement.configuration.is_empty()
    {
        return Err(
            "planned graphics presentation identity does not match its installation".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
fn sink_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    if !matches!(
        placement.kind_id.as_str(),
        "conduit.test/presentation-sink" | "conduit.test/graphics-sink"
    ) {
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
