use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::IMAGE_TEXT_RECORD_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct ImageTextRecordOperation {
    pending: bool,
    complete: bool,
}

impl ImageTextRecordOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.complete => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(
                        value,
                        conduit_net::MAXIMUM_TYPED_RECORD_PAYLOAD_BYTES as u32,
                    ) {
                        Ok(value) => value,
                        Err(_) => return InstalledOperation::fail(159),
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
                    return InstalledOperation::fail(160);
                };
                self.pending = false;
                self.complete = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(161),
        }
    }
    pub(super) fn cancel(&mut self) {
        self.pending = false;
        self.complete = true;
    }
}

pub(super) struct ImageTextRecordHost {
    input_type: StructuredInfoType,
    input_type_bytes: Vec<u8>,
    value_kind: String,
    output_type_bytes: Vec<u8>,
    output: Vec<u8>,
}

impl ImageTextRecordHost {
    fn new() -> Self {
        let input_type = conduit_semantic_catalog::image_text_record_type();
        let input_type_bytes = input_type
            .canonical_bytes()
            .expect("image-text type is finite");
        let value_kind = input_type
            .profile()
            .expect("image-text type has a profile")
            .value_kind()
            .as_str()
            .to_owned();
        Self {
            input_type,
            input_type_bytes,
            value_kind,
            output_type_bytes: conduit_net::typed_record_type()
                .canonical_bytes()
                .expect("typed-record type is finite"),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    pub(super) fn execute(&mut self, input: &[u8]) -> Result<&[u8], &'static str> {
        if input.len() > conduit_net::MAXIMUM_TYPED_RECORD_PAYLOAD_BYTES {
            return Err("image-text payload is too large");
        }
        let node = input
            .strip_prefix(self.input_type_bytes.as_slice())
            .ok_or("wrong image-text type")?;
        self.input_type
            .validate_canonical_node(node)
            .map_err(|_| "malformed image-text value")?;
        if self.value_kind.is_empty()
            || self.value_kind.len() > conduit_net::MAXIMUM_TYPED_RECORD_KIND_BYTES
        {
            return Err("image-text profile is not transportable");
        }
        let leaf_len = 2 + self.value_kind.len() + input.len();
        if leaf_len > conduit_core::MAXIMUM_STRUCTURED_LEAF_BYTES {
            return Err("typed record leaf is too large");
        }
        self.output.clear();
        self.output.extend_from_slice(&self.output_type_bytes);
        self.output.push(0);
        self.output
            .extend_from_slice(&(leaf_len as u32).to_le_bytes());
        self.output
            .extend_from_slice(&(self.value_kind.len() as u16).to_le_bytes());
        self.output.extend_from_slice(self.value_kind.as_bytes());
        self.output.extend_from_slice(input);
        Ok(&self.output)
    }
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Vec<Option<ImageTextRecordHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::IMAGE_TEXT_RECORD_STD_IMPLEMENTATION)
                .then(ImageTextRecordHost::new)
        })
        .collect()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::image_text_record_std_offer();
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
        return Err("planned image-text record adapter differs from installed realization".into());
    }
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        host_requests: 1,
        sign_items: 24,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::ImageTextRecord(
        ImageTextRecordOperation {
            pending: false,
            complete: false,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
        ResourceSemanticIdentity, ResourceVersionIdentity, StructuredInfoValue,
    };

    fn record_value() -> StructuredInfoValue {
        let profile = kind_id("media/image-rgba8@1");
        let image = conduit_human::ImageObservationReference::new(
            BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest([31; 32]),
                content_profile: profile.clone(),
                access_class: ResourceClassId::from("conduit.resource/portable-content@1"),
                extent: ResourceExtent {
                    bytes: 4_096,
                    items: Some(1),
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest([32; 32]),
                    expires_at: None,
                },
            },
            32,
            24,
            &profile,
        )
        .unwrap();
        let record =
            conduit_human::compose_image_text(&profile, image, "Harbor".into(), vec![]).unwrap();
        conduit_semantic_catalog::image_text_record_value(&record, &profile).unwrap()
    }

    #[test]
    fn adapter_emits_the_shared_exact_typed_record_value() {
        let value = record_value();
        let canonical = value.canonical_bytes().unwrap();
        let expected = conduit_net::typed_record_value(&value)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let mut host = ImageTextRecordHost::new();
        assert_eq!(host.execute(&canonical).unwrap(), expected);
    }

    #[test]
    fn adapter_rejects_malformed_exact_type_payload() {
        let value = record_value();
        let mut canonical = value.canonical_bytes().unwrap();
        canonical.push(0);
        let mut host = ImageTextRecordHost::new();
        assert_eq!(host.execute(&canonical), Err("malformed image-text value"));
        assert!(host.output.is_empty());
    }
}
