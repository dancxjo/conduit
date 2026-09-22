//! Explicit finite image-resource provider for hosted continuous Vision.
//!
//! The semantic `ImageResource` carries a reference, never resident pixels.
//! This adapter resolves only an exact installed generation and feeds its
//! bounded grayscale bytes into the pre-admitted continuous local-CV workspace.

use conduit_core::{
    BoundedResourceRef, StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
    RESOURCE_REFERENCE_INFO_ID,
};
use conduit_semantic_catalog::{
    image_resource_type, ContinuousLocalVision, ContinuousLocalVisionObservation,
    ContinuousLocalVisionRefusal, MAXIMUM_LOCAL_CV_PIXELS,
};

mod describe;
mod ocr;

pub const MAXIMUM_HOSTED_VISION_RESOURCES: usize = 8;

pub struct FiniteHostedVisionBase {
    provider_instance_id: String,
    provider: FiniteVisionProvider,
    observation_images: Vec<Vec<u8>>,
    workspace: ContinuousLocalVision,
    last_frame: Option<usize>,
    last_observation: Option<ContinuousLocalVisionObservation>,
    motion_encoder: conduit_semantic_catalog::PreparedLocalVisionMotionEncoder,
    object_encoder: conduit_semantic_catalog::PreparedLocalVisionObjectEncoder,
    ocr_provider: Option<crate::TesseractOcrProvider>,
    ocr_output: Vec<u8>,
    describe: Option<describe::VisualDescriptionState>,
    minimum_motion_delta: u8,
    component_threshold: u8,
    minimum_component_area: u32,
}

impl FiniteHostedVisionBase {
    pub fn new(
        frames: Vec<HostedVisionFrame>,
        width: u16,
        height: u16,
        maximum_components: usize,
        provider_instance_id: impl Into<String>,
    ) -> Result<Self, HostedVisionRefusal> {
        Self::prepare(
            frames,
            width,
            height,
            maximum_components,
            provider_instance_id.into(),
            None,
            None,
        )
    }

    pub fn new_with_ocr(
        frames: Vec<HostedVisionFrame>,
        width: u16,
        height: u16,
        maximum_components: usize,
        provider_instance_id: impl Into<String>,
        ocr_provider: crate::TesseractOcrProvider,
    ) -> Result<Self, HostedVisionRefusal> {
        Self::prepare(
            frames,
            width,
            height,
            maximum_components,
            provider_instance_id.into(),
            Some(ocr_provider),
            None,
        )
    }

    pub fn with_visual_model(
        mut self,
        visual_model: impl crate::hosted_local_model::HostedVisualModelAdapter + 'static,
    ) -> Self {
        self.describe = Some(describe::VisualDescriptionState::new(Box::new(
            visual_model,
        )));
        self
    }

    fn prepare(
        frames: Vec<HostedVisionFrame>,
        width: u16,
        height: u16,
        maximum_components: usize,
        provider_instance_id: String,
        ocr_provider: Option<crate::TesseractOcrProvider>,
        visual_model: Option<Box<dyn crate::hosted_local_model::HostedVisualModelAdapter>>,
    ) -> Result<Self, HostedVisionRefusal> {
        if provider_instance_id.is_empty()
            || provider_instance_id.len()
                > conduit_semantic_catalog::MAXIMUM_LOCAL_VISION_IDENTITY_BYTES
        {
            return Err(HostedVisionRefusal::InvalidOutput);
        }
        let observation_images = frames
            .iter()
            .map(|frame| {
                let image = conduit_human::ImageObservationReference::new(
                    frame.resource.clone(),
                    frame.width,
                    frame.height,
                    &frame.resource.content_profile,
                )
                .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
                conduit_semantic_catalog::image_observation_value(&image)
                    .and_then(|value| {
                        value
                            .canonical_bytes()
                            .map_err(|_| conduit_semantic_catalog::ImageTextValueRefusal::Malformed)
                    })
                    .map_err(|_| HostedVisionRefusal::InvalidOutput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let provider = FiniteVisionProvider::new(frames)?;
        let motion_encoder = conduit_semantic_catalog::PreparedLocalVisionMotionEncoder::new(
            conduit_std_offers::LOCAL_VISION_IMPLEMENTATION,
            provider_instance_id.clone(),
            conduit_std_offers::LOCAL_VISION_ARTIFACT,
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let object_encoder = conduit_semantic_catalog::PreparedLocalVisionObjectEncoder::new(
            conduit_std_offers::LOCAL_VISION_IMPLEMENTATION,
            provider_instance_id.clone(),
            conduit_std_offers::LOCAL_VISION_ARTIFACT,
            width,
            height,
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        Ok(Self {
            provider_instance_id,
            provider,
            observation_images,
            workspace: ContinuousLocalVision::new(width, height, maximum_components)
                .map_err(HostedVisionRefusal::LocalCv)?,
            last_frame: None,
            last_observation: None,
            motion_encoder,
            object_encoder,
            ocr_provider,
            ocr_output: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            describe: visual_model.map(describe::VisualDescriptionState::new),
            minimum_motion_delta: 32,
            component_threshold: 128,
            minimum_component_area: 2,
        })
    }

    pub fn provider_instance_id(&self) -> &str {
        &self.provider_instance_id
    }

    pub fn resource_offer() -> conduit_core::ResourceOffer {
        conduit_core::resource_offer(
            "std-finite-image-residence",
            conduit_std_offers::LOCAL_VISION_RESOURCE_CLASS,
            1,
        )
    }

    pub fn motion_offer() -> conduit_core::CapabilityOffer {
        conduit_std_offers::local_vision_offers()
            .into_iter()
            .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::VISION_MOTION_KIND)
            .expect("reviewed local motion offer")
    }

    pub fn objects_offer() -> conduit_core::CapabilityOffer {
        conduit_std_offers::local_vision_offers()
            .into_iter()
            .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::VISION_OBJECTS_KIND)
            .expect("reviewed local objects offer")
    }

    pub fn ocr_offer(&self) -> Option<conduit_core::CapabilityOffer> {
        self.ocr_provider.as_ref().map(|_| {
            conduit_std_offers::local_vision_offers()
                .into_iter()
                .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::VISION_OCR_KIND)
                .expect("reviewed local OCR offer")
        })
    }

    pub fn describe_offer(&self) -> Option<conduit_core::CapabilityOffer> {
        self.describe.as_ref().map(|_| {
            conduit_std_offers::local_vision_offers()
                .into_iter()
                .find(|offer| {
                    offer.kind_id.as_str() == conduit_semantic_catalog::VISION_DESCRIBE_KIND
                })
                .expect("reviewed visual description offer")
        })
    }

    pub(crate) fn execute_motion(
        &mut self,
        input: &[u8],
        run_id: &str,
    ) -> Result<&[u8], HostedVisionRefusal> {
        let (frame_index, pixels) = self.provider.resolve_exact_canonical(input)?;
        if self.last_frame != Some(frame_index) {
            self.last_observation = Some(
                self.workspace
                    .observe(
                        pixels,
                        self.minimum_motion_delta,
                        self.component_threshold,
                        self.minimum_component_area,
                    )
                    .map_err(HostedVisionRefusal::LocalCv)?,
            );
            self.last_frame = Some(frame_index);
        }
        let observation = self
            .last_observation
            .as_ref()
            .ok_or(HostedVisionRefusal::InvalidOutput)?;
        self.motion_encoder
            .encode(input, observation, run_id)
            .map_err(|_| HostedVisionRefusal::InvalidOutput)
    }

    pub(crate) fn execute_objects(
        &mut self,
        input: &[u8],
        run_id: &str,
        observed_at_micros: u64,
        clock_basis: &str,
    ) -> Result<&[u8], HostedVisionRefusal> {
        let (frame_index, pixels) = self.provider.resolve_exact_canonical(input)?;
        if self.last_frame != Some(frame_index) {
            self.last_observation = Some(
                self.workspace
                    .observe(
                        pixels,
                        self.minimum_motion_delta,
                        self.component_threshold,
                        self.minimum_component_area,
                    )
                    .map_err(HostedVisionRefusal::LocalCv)?,
            );
            self.last_frame = Some(frame_index);
        }
        let observation = self
            .last_observation
            .as_ref()
            .ok_or(HostedVisionRefusal::InvalidOutput)?;
        self.object_encoder
            .encode(
                self.observation_images
                    .get(frame_index)
                    .ok_or(HostedVisionRefusal::InvalidOutput)?,
                observation,
                run_id,
                observed_at_micros,
                clock_basis,
            )
            .map_err(|_| HostedVisionRefusal::InvalidOutput)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedVisionFrame {
    pub canonical_image: Vec<u8>,
    pub resource: BoundedResourceRef,
    pub width: u16,
    pub height: u16,
    pub grayscale_pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedVisionRefusal {
    MalformedImageResource,
    UnavailableResourceGeneration,
    ResourceShapeMismatch,
    ResourceCapacity,
    InvalidOutput,
    LocalCv(ContinuousLocalVisionRefusal),
    Ocr(crate::OcrProviderRefusal),
    VisualModel,
}

pub trait HostedVisionProvider: Send {
    fn resolve_exact(
        &mut self,
        resource: &BoundedResourceRef,
        width: u16,
        height: u16,
    ) -> Result<&[u8], HostedVisionRefusal>;
}

/// A finite explicit resource residence used by the first std realization.
/// Construction installs every available generation before Play.
pub struct FiniteVisionProvider {
    frames: Vec<HostedVisionFrame>,
}

impl FiniteVisionProvider {
    pub fn new(frames: Vec<HostedVisionFrame>) -> Result<Self, HostedVisionRefusal> {
        if frames.is_empty() || frames.len() > MAXIMUM_HOSTED_VISION_RESOURCES {
            return Err(HostedVisionRefusal::ResourceCapacity);
        }
        for (index, frame) in frames.iter().enumerate() {
            frame
                .resource
                .validate()
                .map_err(|_| HostedVisionRefusal::MalformedImageResource)?;
            let pixels = usize::from(frame.width)
                .checked_mul(usize::from(frame.height))
                .filter(|count| *count > 0 && *count <= MAXIMUM_LOCAL_CV_PIXELS)
                .ok_or(HostedVisionRefusal::ResourceShapeMismatch)?;
            if frame.grayscale_pixels.len() != pixels
                || frames[..index]
                    .iter()
                    .any(|prior| prior.resource == frame.resource)
            {
                return Err(HostedVisionRefusal::ResourceShapeMismatch);
            }
            let (_, canonical_resource, canonical_width, canonical_height) =
                decode_image_resource(&frame.canonical_image)?;
            if canonical_resource != frame.resource
                || canonical_width != frame.width
                || canonical_height != frame.height
            {
                return Err(HostedVisionRefusal::ResourceShapeMismatch);
            }
        }
        Ok(Self { frames })
    }

    fn resolve_exact_canonical(
        &self,
        encoded: &[u8],
    ) -> Result<(usize, &[u8]), HostedVisionRefusal> {
        self.frames
            .iter()
            .enumerate()
            .find(|(_, frame)| frame.canonical_image == encoded)
            .map(|(index, frame)| (index, frame.grayscale_pixels.as_slice()))
            .ok_or(HostedVisionRefusal::UnavailableResourceGeneration)
    }
}

impl HostedVisionProvider for FiniteVisionProvider {
    fn resolve_exact(
        &mut self,
        resource: &BoundedResourceRef,
        width: u16,
        height: u16,
    ) -> Result<&[u8], HostedVisionRefusal> {
        let frame = self
            .frames
            .iter()
            .find(|frame| &frame.resource == resource)
            .ok_or(HostedVisionRefusal::UnavailableResourceGeneration)?;
        if frame.width != width || frame.height != height {
            return Err(HostedVisionRefusal::ResourceShapeMismatch);
        }
        Ok(&frame.grayscale_pixels)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedVisionObservation {
    pub source: BoundedResourceRef,
    pub source_image: StructuredInfoValue,
    pub width: u16,
    pub height: u16,
    pub local: ContinuousLocalVisionObservation,
}

pub struct HostedContinuousVision<P> {
    provider: P,
    workspace: ContinuousLocalVision,
    width: u16,
    height: u16,
    last: Option<HostedVisionObservation>,
    #[cfg(test)]
    output: Vec<u8>,
}

impl<P: HostedVisionProvider> HostedContinuousVision<P> {
    pub fn new(
        provider: P,
        width: u16,
        height: u16,
        maximum_components: usize,
    ) -> Result<Self, HostedVisionRefusal> {
        Ok(Self {
            provider,
            workspace: ContinuousLocalVision::new(width, height, maximum_components)
                .map_err(HostedVisionRefusal::LocalCv)?,
            width,
            height,
            last: None,
            #[cfg(test)]
            output: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub fn observe_image_resource(
        &mut self,
        encoded: &[u8],
        minimum_motion_delta: u8,
        component_threshold: u8,
        minimum_component_area: u32,
    ) -> Result<&HostedVisionObservation, HostedVisionRefusal> {
        let (source_image, source, width, height) = decode_image_resource(encoded)?;
        if width != self.width || height != self.height {
            return Err(HostedVisionRefusal::ResourceShapeMismatch);
        }
        if self.last.as_ref().is_some_and(|last| last.source == source) {
            return Ok(self.last.as_ref().expect("checked cached observation"));
        }
        let pixels = self.provider.resolve_exact(&source, width, height)?;
        let local = self
            .workspace
            .observe(
                pixels,
                minimum_motion_delta,
                component_threshold,
                minimum_component_area,
            )
            .map_err(HostedVisionRefusal::LocalCv)?;
        self.last = Some(HostedVisionObservation {
            source,
            source_image,
            width,
            height,
            local,
        });
        Ok(self.last.as_ref().expect("observation was just installed"))
    }

    #[cfg(test)]
    fn observe_motion_encoded(
        &mut self,
        encoded: &[u8],
        minimum_motion_delta: u8,
        component_threshold: u8,
        minimum_component_area: u32,
        provenance: &conduit_semantic_catalog::LocalVisionProvenance,
    ) -> Result<&[u8], HostedVisionRefusal> {
        let observation = self
            .observe_image_resource(
                encoded,
                minimum_motion_delta,
                component_threshold,
                minimum_component_area,
            )?
            .clone();
        let output = conduit_semantic_catalog::local_vision_motion_observation_value(
            observation.source_image,
            &observation.local,
            provenance,
        )
        .and_then(|value| value.canonical_bytes().map_err(Into::into))
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        if output.len() > self.output.capacity() {
            return Err(HostedVisionRefusal::InvalidOutput);
        }
        self.output.clear();
        self.output.extend_from_slice(&output);
        Ok(&self.output)
    }

    pub fn storage(&self) -> conduit_semantic_catalog::ContinuousLocalVisionStorage {
        self.workspace.storage()
    }
}

fn decode_image_resource(
    encoded: &[u8],
) -> Result<(StructuredInfoValue, BoundedResourceRef, u16, u16), HostedVisionRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| HostedVisionRefusal::MalformedImageResource)?;
    if value.value_type() != &image_resource_type() {
        return Err(HostedVisionRefusal::MalformedImageResource);
    }
    let content = record_field(&value, "content")?;
    if !matches!(
        content.value_type().shape(),
        StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == RESOURCE_REFERENCE_INFO_ID
    ) {
        return Err(HostedVisionRefusal::MalformedImageResource);
    }
    let StructuredInfoValueShape::Leaf(reference) = content.shape() else {
        return Err(HostedVisionRefusal::MalformedImageResource);
    };
    let resource = BoundedResourceRef::decode(reference)
        .map_err(|_| HostedVisionRefusal::MalformedImageResource)?;
    let extent = record_field(&value, "extent")?;
    let width = count(record_field(extent, "width")?)?;
    let height = count(record_field(extent, "height")?)?;
    Ok((value, resource, width, height))
}

#[cfg(test)]
pub(crate) fn decode_image_resource_for_test(
    encoded: &[u8],
) -> (StructuredInfoValue, BoundedResourceRef, u16, u16) {
    decode_image_resource(encoded).expect("test image resource is canonical")
}

fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, HostedVisionRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(HostedVisionRefusal::MalformedImageResource);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value())
        .ok_or(HostedVisionRefusal::MalformedImageResource)
}

fn count(value: &StructuredInfoValue) -> Result<u16, HostedVisionRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(HostedVisionRefusal::MalformedImageResource);
    };
    conduit_core::decode_count(bytes)
        .ok()
        .and_then(|value| u16::try_from(value).ok())
        .ok_or(HostedVisionRefusal::MalformedImageResource)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_semantic_catalog::deterministic_vision_fixture;

    fn reference(encoded: &[u8]) -> BoundedResourceRef {
        decode_image_resource(encoded).unwrap().1
    }

    #[test]
    fn exact_generation_resolves_once_and_reuses_one_continuous_observation() {
        let image = deterministic_vision_fixture().unwrap().image;
        let encoded = image.canonical_bytes().unwrap();
        let source = reference(&encoded);
        let mut pixels = vec![0; 64 * 48];
        pixels[65] = 255;
        pixels[66] = 255;
        let provider = FiniteVisionProvider::new(vec![HostedVisionFrame {
            canonical_image: encoded.clone(),
            resource: source,
            width: 64,
            height: 48,
            grayscale_pixels: pixels,
        }])
        .unwrap();
        let mut vision = HostedContinuousVision::new(provider, 64, 48, 4).unwrap();
        let admitted = vision.storage();
        let first = vision
            .observe_image_resource(&encoded, 32, 128, 2)
            .unwrap()
            .clone();
        let second = vision
            .observe_image_resource(&encoded, 32, 128, 2)
            .unwrap()
            .clone();
        assert_eq!(first, second);
        assert_eq!(first.local.sequence, 1);
        assert_eq!(first.local.component_count, 1);
        assert_eq!(vision.storage(), admitted);
    }

    #[test]
    fn unknown_generation_and_shape_mismatch_refuse_without_fallback() {
        let fixture = deterministic_vision_fixture().unwrap();
        let encoded = fixture.image.canonical_bytes().unwrap();
        let installed = reference(&encoded);
        let mut unavailable = installed.clone();
        unavailable.lifetime.version =
            conduit_core::ResourceVersionIdentity::from_digest([0x33; 32]);
        let mut provider = FiniteVisionProvider::new(vec![HostedVisionFrame {
            canonical_image: encoded.clone(),
            resource: installed,
            width: 64,
            height: 48,
            grayscale_pixels: vec![0; 64 * 48],
        }])
        .unwrap();
        assert_eq!(
            provider.resolve_exact(&unavailable, 64, 48),
            Err(HostedVisionRefusal::UnavailableResourceGeneration)
        );
        let mut wrong_shape = HostedContinuousVision::new(provider, 32, 32, 4).unwrap();
        assert_eq!(
            wrong_shape.observe_image_resource(&encoded, 32, 128, 2),
            Err(HostedVisionRefusal::ResourceShapeMismatch)
        );
    }

    #[test]
    fn motion_output_is_canonical_typed_and_keeps_the_exact_image_generation() {
        let image = deterministic_vision_fixture().unwrap().image;
        let encoded = image.canonical_bytes().unwrap();
        let provider = FiniteVisionProvider::new(vec![HostedVisionFrame {
            canonical_image: encoded.clone(),
            resource: reference(&encoded),
            width: 64,
            height: 48,
            grayscale_pixels: vec![0; 64 * 48],
        }])
        .unwrap();
        let mut vision = HostedContinuousVision::new(provider, 64, 48, 4).unwrap();
        let output = vision
            .observe_motion_encoded(
                &encoded,
                32,
                128,
                2,
                &conduit_semantic_catalog::LocalVisionProvenance {
                    implementation_id: conduit_std_offers::LOCAL_VISION_IMPLEMENTATION.into(),
                    provider_instance_id: "finite-image-residence/test-1".into(),
                    artifact_id: conduit_std_offers::LOCAL_VISION_ARTIFACT.into(),
                    run_id: "play/test-1/sequence-1".into(),
                },
            )
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(output).unwrap();
        assert_eq!(
            value.value_type(),
            &conduit_semantic_catalog::vision_motions_type()
        );
        assert_eq!(vision.storage().previous_pixels, 64 * 48);
    }

    #[test]
    fn object_output_is_typed_and_reuses_the_exact_cached_observation() {
        let image = deterministic_vision_fixture().unwrap().image;
        let encoded = image.canonical_bytes().unwrap();
        let mut pixels = vec![0; 64 * 48];
        pixels[65] = 255;
        pixels[66] = 255;
        let mut vision = FiniteHostedVisionBase::new(
            vec![HostedVisionFrame {
                canonical_image: encoded.clone(),
                resource: reference(&encoded),
                width: 64,
                height: 48,
                grayscale_pixels: pixels,
            }],
            64,
            48,
            4,
            "finite-image-residence/object-test",
        )
        .unwrap();
        let objects = vision
            .execute_objects(&encoded, "play/test/request-1", 41, "boot/test/monotonic")
            .unwrap()
            .to_vec();
        let object_value = StructuredInfoValue::from_canonical_bytes(&objects).unwrap();
        assert_eq!(
            object_value.value_type(),
            &conduit_semantic_catalog::vision_objects_type()
        );
        let profile = reference(&encoded).content_profile;
        let decoded =
            conduit_semantic_catalog::object_observations_from_value(&object_value, &profile)
                .unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].candidate_label, "bright-component");
        assert_eq!(decoded[0].provenance.observed_at.ticks, 41);
        assert_eq!(
            decoded[0].provenance.observation_sign_id.as_str(),
            "play/test/request-1/observation-0"
        );
        vision
            .execute_motion(&encoded, "play/test/request-2")
            .unwrap();
        assert_eq!(vision.last_observation.unwrap().sequence, 1);
    }
}
