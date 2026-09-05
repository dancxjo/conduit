use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, StructuredInfoValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::IMAGE_TEXT_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct ImageTextOperation {
    pending: Option<RequestId>,
    next: u32,
    complete: bool,
}

impl ImageTextOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value }
                if self.pending.is_none() && !self.complete && port.0 < 2 =>
            {
                let request = RequestId(self.next);
                self.next = self.next.saturating_add(1);
                self.pending = Some(request);
                let maximum = if port == PortId(0) {
                    MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
                } else {
                    conduit_human::MAXIMUM_IMAGE_TEXT_CAPTION_BYTES as u32
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(port.0),
                    input: match BoundedValueRef::new(value, maximum) {
                        Ok(value) => value,
                        Err(_) => return InstalledOperation::fail(157),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                match outcome.output {
                    Some(output) => {
                        self.complete = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    None => OperationAction::Await,
                }
            }
            OperationInput::Closed { .. } if self.pending.is_none() => OperationAction::Complete,
            _ => InstalledOperation::fail(158),
        }
    }
    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.complete = true;
    }
}

pub(super) struct ImageTextHost {
    image: Option<conduit_human::ImageObservationReference>,
    caption: Option<String>,
    output: Vec<u8>,
}

impl ImageTextHost {
    fn new() -> Self {
        Self {
            image: None,
            caption: None,
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }
    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<&[u8]>, String> {
        match contract {
            conduit_std_offers::IMAGE_TEXT_IMAGE_OPERATION => {
                let value = StructuredInfoValue::from_canonical_bytes(input)
                    .map_err(|error| format!("image value: {error:?}"))?;
                self.image = Some(
                    conduit_semantic_catalog::image_observation_from_value(&value)
                        .map_err(|error| format!("image observation: {error:?}"))?,
                );
            }
            conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION => {
                self.caption = Some(
                    core::str::from_utf8(input)
                        .map_err(|_| "caption is not UTF-8")?
                        .to_owned(),
                );
            }
            _ => return Err("unknown image-text host operation".into()),
        }
        let (Some(image), Some(caption)) = (&self.image, &self.caption) else {
            return Ok(None);
        };
        let profile = image.content.content_profile.clone();
        let record =
            conduit_human::compose_image_text(&profile, image.clone(), caption.clone(), vec![])
                .map_err(|error| format!("compose image text: {error:?}"))?;
        let value = conduit_semantic_catalog::image_text_record_value(&record, &profile)
            .map_err(|error| format!("encode image text: {error:?}"))?;
        self.output = value
            .canonical_bytes()
            .map_err(|error| format!("canonical image text: {error:?}"))?;
        Ok(Some(&self.output))
    }
}

pub(super) fn prepare_hosts(fragment: &conduit_core::PlanFragment) -> Vec<Option<ImageTextHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::IMAGE_TEXT_STD_IMPLEMENTATION)
                .then(ImageTextHost::new)
        })
        .collect()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::image_text_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
    {
        return Err("planned image-text operation differs from installed realization".into());
    }
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
        host_requests: 2,
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::ImageText(ImageTextOperation {
        pending: None,
        next: 0,
        complete: false,
    }))
}
