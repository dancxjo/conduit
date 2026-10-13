use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_face::{
    FaceComposition, GraphicsScene, MAX_FACE_COMPOSITION_BYTES, MAX_GRAPHICS_SCENE_BYTES,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef, ValueStorage,
};

macro_rules! factory {
    ($name:ident, $implementation:ident) => {
        pub(super) static $name: BackFactory = BackFactory {
            implementation_id: conduit_std_offers::$implementation,
            budget,
            prepare,
        };
    };
}
factory!(FACE_ICON_FACTORY, FACE_ICON_IMPLEMENTATION);
factory!(FACE_FRAME_FACTORY, FACE_FRAME_IMPLEMENTATION);
factory!(FACE_BADGE_FACTORY, FACE_BADGE_IMPLEMENTATION);
factory!(GRAPHICS_RECT_FACTORY, GRAPHICS_RECT_IMPLEMENTATION);
factory!(GRAPHICS_TEXT_FACTORY, GRAPHICS_TEXT_IMPLEMENTATION);
factory!(GRAPHICS_ICON_FACTORY, GRAPHICS_ICON_IMPLEMENTATION);
pub(super) static GRAPHICS_FACE_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::GRAPHICS_FACE_IMPLEMENTATION,
    budget: graphics_face_budget,
    prepare: prepare_graphics_face,
};
#[cfg(test)]
pub(super) static TEST_FACE_SINK_FACTORY: BackFactory = BackFactory {
    implementation_id: "conduit-test/face-sink-implementation@1",
    budget: sink_budget,
    prepare: prepare_sink,
};
#[cfg(test)]
pub(super) static TEST_GRAPHICS_SINK_FACTORY: BackFactory = BackFactory {
    implementation_id: "conduit-test/graphics-sink-implementation@1",
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct FaceCompositionBack {
    source: Option<ValueRef>,
    pending: bool,
    emitted: bool,
}

pub(super) struct GraphicsFaceBack {
    pending: bool,
    presented: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for GraphicsFaceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if request != RequestId(0) || !self.pending {
                return StepOutcome::Fail(step_failure(48));
            }
            io.consume_host_completion()
                .expect("observed graphics Face completion");
            self.pending = false;
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.output.is_some() {
                return StepOutcome::Fail(step_failure(48));
            }
            self.presented = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending || self.presented {
                return StepOutcome::Fail(step_failure(48));
            }
            let Ok(input) = BoundedValueRef::new(value, MAX_GRAPHICS_SCENE_BYTES as u32) else {
                return StepOutcome::Fail(step_failure(48));
            };
            io.consume(PortId(0)).expect("present graphics Face input");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("graphics Face Host Call");
            self.pending = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && !self.pending {
            io.consume_closed(PortId(0))
                .expect("observed graphics Face closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

impl GraphicsFaceBack {}

#[cfg(test)]
pub(super) struct FaceSinkBack;

#[cfg(test)]
impl<const PORTS: usize> StepBack<PORTS> for FaceSinkBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            io.consume(PortId(0)).expect("present test Face input");
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed test Face closure");
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}

#[cfg(test)]
impl FaceSinkBack {}

impl FaceCompositionBack {}

impl<const PORTS: usize> StepBack<PORTS> for FaceCompositionBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = self.source {
            if self.emitted {
                return StepOutcome::Complete;
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.send(PortId(0), value)
                .expect("ready Face composition source output");
            self.emitted = true;
            return StepOutcome::Progress;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if request != RequestId(0)
                || !self.pending
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return StepOutcome::Fail(step_failure(46));
            }
            let Some(output) = outcome.output else {
                return StepOutcome::Fail(step_failure(45));
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_host_completion()
                .expect("observed Face composition completion");
            io.send(PortId(0), output.value)
                .expect("ready Face composition output");
            self.pending = false;
            self.emitted = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending || self.emitted {
                return StepOutcome::Fail(step_failure(46));
            }
            let Ok(input) = BoundedValueRef::new(value, MAX_FACE_COMPOSITION_BYTES as u32) else {
                return StepOutcome::Fail(step_failure(44));
            };
            io.consume(PortId(0))
                .expect("present Face composition input");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("Face composition Host Call");
            self.pending = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && !self.pending {
            io.consume_closed(PortId(0))
                .expect("observed Face composition closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
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
) -> Result<([u8; MAX_FACE_COMPOSITION_BYTES], usize), String> {
    let value = FaceComposition::decode(input)
        .map_err(|error| format!("decode face composition: {error:?}"))?;
    let output = conduit_semantic_catalog::execute_face_transform(placement, value)?;
    Ok((output.encode(), output.encoded_len()))
}

pub(super) fn transform_graphics_bytes(
    placement: &PlannedGear,
    input: &[u8],
) -> Result<([u8; MAX_GRAPHICS_SCENE_BYTES], usize), String> {
    let (composition, scene) =
        if placement.kind_id.as_str() == conduit_semantic_catalog::GRAPHICS_RECT_KIND {
            let composition = FaceComposition::decode(input)
                .map_err(|error| format!("decode face composition: {error:?}"))?;
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
    let scene =
        conduit_semantic_catalog::execute_graphics_transform(placement, composition, scene)?;
    Ok((scene.encode(), scene.encoded_len()))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: MAX_FACE_COMPOSITION_BYTES.max(MAX_GRAPHICS_SCENE_BYTES) as u32,
        host_requests: usize::from(
            placement.kind_id.as_str() != conduit_semantic_catalog::FACE_ICON_KIND,
        ),
        sign_items: 32,
        maximum_value_bytes: MAX_FACE_COMPOSITION_BYTES.max(MAX_GRAPHICS_SCENE_BYTES) as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    let source = if placement.kind_id.as_str() == conduit_semantic_catalog::FACE_ICON_KIND {
        let value = conduit_semantic_catalog::execute_face_source(placement)?;
        let bytes = value.encode();
        Some(
            values
                .store(&bytes[..value.encoded_len()])
                .map_err(|error| format!("store face composition: {error:?}"))?,
        )
    } else {
        None
    };
    Ok(InstalledBack::FaceComposition(FaceCompositionBack {
        source,
        pending: false,
        emitted: false,
    }))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::face_composition_offer_for(placement.kind_id.as_str())
        .or_else(|| conduit_std_offers::graphics_offer_for(placement.kind_id.as_str()))
        .ok_or_else(|| "unsupported face or graphics Kind".to_string())?;
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
        return Err("planned face composition identity does not match its installation".into());
    }
    Ok(())
}

fn graphics_face_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_graphics_face(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: MAX_GRAPHICS_SCENE_BYTES as u32,
    })
}

fn prepare_graphics_face(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate_graphics_face(placement)?;
    Ok(InstalledBack::GraphicsFace(GraphicsFaceBack {
        pending: false,
        presented: false,
    }))
}

fn validate_graphics_face(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::graphics_face_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str() != conduit_core::FACE_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || placement.configuration.len() != 1
    {
        return Err("planned graphics face identity does not match its installation".into());
    }
    Ok(())
}

#[cfg(test)]
fn sink_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    if !matches!(
        placement.kind_id.as_str(),
        "conduit-test/face-sink" | "conduit-test/graphics-sink"
    ) {
        return Err("wrong face sink Kind".into());
    }
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: MAX_FACE_COMPOSITION_BYTES as u32,
    })
}

#[cfg(test)]
fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    sink_budget(placement)?;
    Ok(InstalledBack::TestFaceSink(FaceSinkBack))
}
