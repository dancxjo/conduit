use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
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
                    operation: HostOperationId(if port == PortId(0) { 1 } else { 0 }),
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
    image_type: Vec<u8>,
    record_type: Vec<u8>,
    image_node: Vec<u8>,
    resource: Vec<u8>,
    dimensions: Option<(u16, u16)>,
    caption: Vec<u8>,
    has_caption: bool,
    digest_input: Vec<u8>,
    output: Vec<u8>,
}

impl ImageTextHost {
    fn new() -> Self {
        Self {
            image_type: conduit_semantic_catalog::image_observation_reference_type()
                .canonical_bytes()
                .expect("reviewed image type remains finite"),
            record_type: conduit_semantic_catalog::image_text_record_type()
                .canonical_bytes()
                .expect("reviewed image-text type remains finite"),
            image_node: Vec::with_capacity(1_024),
            resource: Vec::with_capacity(conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES),
            dimensions: None,
            caption: Vec::with_capacity(conduit_human::MAXIMUM_IMAGE_TEXT_CAPTION_BYTES),
            has_caption: false,
            digest_input: Vec::with_capacity(1_100),
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
                let parsed = parse_image(input, &self.image_type).map_err(String::from)?;
                let ParsedImage {
                    node,
                    resource,
                    width,
                    height,
                } = parsed;
                let reference = conduit_core::BoundedResourceRef::validate_encoded(resource)
                    .map_err(|error| format!("image resource: {error:?}"))?;
                if reference.extent.bytes > conduit_human::MAXIMUM_IMAGE_OBSERVATION_BYTES
                    || width == 0
                    || height == 0
                    || width > conduit_human::MAXIMUM_IMAGE_OBSERVATION_WIDTH
                    || height > conduit_human::MAXIMUM_IMAGE_OBSERVATION_HEIGHT
                {
                    return Err("image observation exceeds its finite profile".into());
                }
                self.image_node.clear();
                self.image_node.extend_from_slice(node);
                self.resource.clear();
                self.resource.extend_from_slice(resource);
                self.dimensions = Some((width, height));
            }
            conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION => {
                if input.is_empty()
                    || input.len() > conduit_human::MAXIMUM_IMAGE_TEXT_CAPTION_BYTES
                    || core::str::from_utf8(input).is_err()
                {
                    return Err("caption is empty, oversized, or not UTF-8".into());
                }
                self.caption.clear();
                self.caption.extend_from_slice(input);
                self.has_caption = true;
            }
            _ => return Err("unknown image-text host operation".into()),
        }
        let Some((width, height)) = self.dimensions else {
            return Ok(None);
        };
        if !self.has_caption {
            return Ok(None);
        }
        self.digest_input.clear();
        self.digest_input.extend_from_slice(&self.resource);
        self.digest_input.extend_from_slice(&width.to_le_bytes());
        self.digest_input.extend_from_slice(&height.to_le_bytes());
        self.digest_input
            .extend_from_slice(&(self.caption.len() as u64).to_le_bytes());
        self.digest_input.extend_from_slice(&self.caption);
        self.digest_input.extend_from_slice(&0_u64.to_le_bytes());
        let digest = conduit_core::semantic_digest("human/image-text-record@1", &self.digest_input);
        self.output.clear();
        self.output.extend_from_slice(&self.record_type);
        record_start(&mut self.output, 4);
        field_leaf(&mut self.output, "caption", &self.caption);
        field_leaf(&mut self.output, "content_digest", &digest);
        text(&mut self.output, "image");
        self.output.extend_from_slice(&self.image_node);
        text(&mut self.output, "metadata");
        self.output.push(1);
        length(
            &mut self.output,
            conduit_human::MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES,
        );
        for _ in 0..conduit_human::MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES {
            self.output.push(3);
            text(&mut self.output, "absent");
            self.output.push(0);
            length(&mut self.output, 0);
        }
        Ok(Some(&self.output))
    }
}

struct ParsedImage<'a> {
    node: &'a [u8],
    resource: &'a [u8],
    width: u16,
    height: u16,
}

fn parse_image<'a>(input: &'a [u8], expected_type: &[u8]) -> Result<ParsedImage<'a>, &'static str> {
    let node = input
        .strip_prefix(expected_type)
        .ok_or("wrong image type")?;
    let mut cursor = CanonicalCursor::new(node);
    cursor.expect(2)?;
    if cursor.length()? != 3 {
        return Err("wrong image field count");
    }
    cursor.text("content")?;
    cursor.expect(0)?;
    let resource = cursor.bytes()?;
    cursor.text("height")?;
    let height = cursor.count()?;
    cursor.text("width")?;
    let width = cursor.count()?;
    if !cursor.remaining.is_empty() {
        return Err("trailing image value");
    }
    Ok(ParsedImage {
        node,
        resource,
        width,
        height,
    })
}

struct CanonicalCursor<'a> {
    remaining: &'a [u8],
}
impl<'a> CanonicalCursor<'a> {
    fn new(remaining: &'a [u8]) -> Self {
        Self { remaining }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], &'static str> {
        let (head, tail) = self
            .remaining
            .split_at_checked(n)
            .ok_or("truncated image value")?;
        self.remaining = tail;
        Ok(head)
    }
    fn expect(&mut self, byte: u8) -> Result<(), &'static str> {
        if self.take(1)? == [byte] {
            Ok(())
        } else {
            Err("wrong image node")
        }
    }
    fn length(&mut self) -> Result<usize, &'static str> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| "bad length")?) as usize)
    }
    fn bytes(&mut self) -> Result<&'a [u8], &'static str> {
        let n = self.length()?;
        self.take(n)
    }
    fn text(&mut self, expected: &str) -> Result<(), &'static str> {
        if self.bytes()? == expected.as_bytes() {
            Ok(())
        } else {
            Err("wrong image field")
        }
    }
    fn count(&mut self) -> Result<u16, &'static str> {
        self.expect(0)?;
        let bytes: [u8; 8] = self.bytes()?.try_into().map_err(|_| "wrong count")?;
        u16::try_from(u64::from_le_bytes(bytes)).map_err(|_| "count exceeds image dimensions")
    }
}

fn length(output: &mut Vec<u8>, value: usize) {
    output.extend_from_slice(&(value as u32).to_le_bytes());
}
fn text(output: &mut Vec<u8>, value: &str) {
    length(output, value.len());
    output.extend_from_slice(value.as_bytes());
}
fn record_start(output: &mut Vec<u8>, fields: usize) {
    output.push(2);
    length(output, fields);
}
fn field_leaf(output: &mut Vec<u8>, name: &str, value: &[u8]) {
    text(output, name);
    output.push(0);
    length(output, value.len());
    output.extend_from_slice(value);
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
        ResourceSemanticIdentity, ResourceVersionIdentity, StructuredInfoValue,
    };

    fn image_value() -> (conduit_core::KindId, Vec<u8>) {
        let profile = kind_id("media/image-rgba8@1");
        let image = conduit_human::ImageObservationReference::new(
            BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest([7; 32]),
                content_profile: profile.clone(),
                access_class: ResourceClassId::from("conduit.resource/portable-content@1"),
                extent: ResourceExtent {
                    bytes: 12_288,
                    items: Some(1),
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest([8; 32]),
                    expires_at: None,
                },
            },
            64,
            48,
            &profile,
        )
        .unwrap();
        let value = conduit_semantic_catalog::image_observation_value(&image).unwrap();
        (profile, value.canonical_bytes().unwrap())
    }

    #[test]
    fn host_composes_after_either_exact_input_order() {
        let (profile, image) = image_value();
        for image_first in [true, false] {
            let mut host = ImageTextHost::new();
            let first = if image_first {
                host.execute(conduit_std_offers::IMAGE_TEXT_IMAGE_OPERATION, &image)
            } else {
                host.execute(
                    conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION,
                    b"At the pier",
                )
            };
            assert!(first.unwrap().is_none());
            let encoded = if image_first {
                host.execute(
                    conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION,
                    b"At the pier",
                )
            } else {
                host.execute(conduit_std_offers::IMAGE_TEXT_IMAGE_OPERATION, &image)
            }
            .unwrap()
            .unwrap()
            .to_vec();
            let value = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
            let record =
                conduit_semantic_catalog::image_text_record_from_value(&value, &profile).unwrap();
            assert_eq!(record.caption, "At the pier");
            assert_eq!(record.image.width, 64);
            assert!(record.metadata.is_empty());
        }
    }

    #[test]
    fn malformed_inputs_refuse_without_manufacturing_a_record() {
        let mut host = ImageTextHost::new();
        assert!(host
            .execute(
                conduit_std_offers::IMAGE_TEXT_IMAGE_OPERATION,
                b"not structured"
            )
            .unwrap_err()
            .contains("image type"));
        assert!(host
            .execute(conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION, &[0xff])
            .unwrap_err()
            .contains("UTF-8"));
        assert!(host.output.is_empty());
    }
}
