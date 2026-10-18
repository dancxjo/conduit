//! Frozen formant projection from the original checked linguistic carrier.
//! This prepares PCM computation, not playback authority or parser commitment.
use crate::{
    correspondence::PreparedSpeechCorrespondence,
    intent_realization::{prepare_intent_realization, IntentRealizationRefusal},
    render::frozen::FrozenRender,
    semantic::{
        SpeechAmplitudeQ15, SpeechCarrierDigest, SpeechCycleQ8AtRate, SpeechEventFrameSpan,
        SpeechFormantBoundaryProfile, SpeechFormantVoiceProfile, SpeechOfflinePcmAdmission,
        SpeechOfflinePcmDeclaration,
    },
    RenderRefusal,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum CarriedFormantRefusal {
    Native(NativeBindingRefusal),
    Realization(IntentRealizationRefusal),
    Render(RenderRefusal),
    UnsupportedPhoneFeatures { phone: usize },
}

/// Owned immutable preparation; no source lookup, pronunciation, allocation or
/// inventory/profile selection remains for a kernel Step. The canonical source
/// bytes remain alive through the final output. Decoding bytes cannot construct
/// this receipt: executed IPA inventory/correspondence admission is required.
pub struct FrozenCarriedFormant {
    pub(crate) render: FrozenRender,
    original: Vec<u8>,
    source_identity: [u8; 32],
    projection: Vec<u8>,
    projection_identity: [u8; 32],
    timing: Vec<SpeechEventFrameSpan>,
    controls: Vec<(Option<SpeechCycleQ8AtRate>, Option<SpeechAmplitudeQ15>)>,
}
impl FrozenCarriedFormant {
    pub fn prepare(
        carrier: &PreparedSpeechCorrespondence<'_, '_>,
        profile: &SpeechFormantVoiceProfile,
        boundaries: &SpeechFormantBoundaryProfile,
    ) -> Result<Self, CarriedFormantRefusal> {
        for (phone, token) in carrier.phones().tokens().as_slice().iter().enumerate() {
            if !token.features().get().as_slice().is_empty() {
                return Err(CarriedFormantRefusal::UnsupportedPhoneFeatures { phone });
            }
        }
        let realized = prepare_intent_realization(
            carrier.intent(),
            carrier.inventory().inventory(),
            profile,
            boundaries,
        )
        .map_err(CarriedFormantRefusal::Realization)?;
        let render = FrozenRender::capture(realized.renderer().map_err(|error| {
            CarriedFormantRefusal::Realization(IntentRealizationRefusal::Renderer(error))
        })?)
        .map_err(CarriedFormantRefusal::Render)?;
        let original = crate::carrier_identity::canonical_carrier_bytes(carrier)
            .map_err(CarriedFormantRefusal::Native)?;
        let source_identity =
            conduit_core::semantic_digest("conduit.speech/checked-carrier@1", &original);
        // Evidence envelope of existing exact values, not another linguistic IR.
        let mut projection = Vec::new();
        for component in [profile.clone().encode(), boundaries.clone().encode()] {
            let bytes = component.map_err(CarriedFormantRefusal::Native)?;
            projection.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            projection.extend_from_slice(&bytes);
        }
        let timing = realized.timing().spans().to_vec();
        for span in &timing {
            let bytes = span
                .clone()
                .encode()
                .map_err(CarriedFormantRefusal::Native)?;
            projection.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            projection.extend_from_slice(&bytes);
        }
        let mut controls = Vec::with_capacity(timing.len());
        for receipt in realized.timing().receipts() {
            let values = match receipt {
                crate::utterance_timing::EventTimingReceipt::Segment { control, .. } => {
                    (control.cycle().cloned(), Some(control.amplitude().clone()))
                }
                crate::utterance_timing::EventTimingReceipt::Boundary { .. } => (None, None),
            };
            for encoded in [
                values.0.clone().map(NativeRustBinding::encode),
                values.1.clone().map(NativeRustBinding::encode),
            ] {
                match encoded {
                    Some(bytes) => {
                        let bytes = bytes.map_err(CarriedFormantRefusal::Native)?;
                        projection.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                        projection.extend_from_slice(&bytes);
                    }
                    None => projection.extend_from_slice(&0_u64.to_le_bytes()),
                }
            }
            controls.push(values);
        }
        projection.extend_from_slice(crate::SOURCE_ID.as_bytes());
        projection.extend_from_slice(&source_identity);
        let projection_identity = conduit_core::semantic_digest(
            "conduit.speech/carried-formant-projection@1",
            &projection,
        );
        Ok(Self {
            render,
            original,
            source_identity,
            projection,
            projection_identity,
            timing,
            controls,
        })
    }
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }
    pub fn source_identity(&self) -> &[u8; 32] {
        &self.source_identity
    }
    pub fn projection_bytes(&self) -> &[u8] {
        &self.projection
    }
    pub fn projection_identity(&self) -> &[u8; 32] {
        &self.projection_identity
    }
    pub fn timing_receipts(&self) -> &[SpeechEventFrameSpan] {
        &self.timing
    }
    pub fn control_receipts(&self) -> &[(Option<SpeechCycleQ8AtRate>, Option<SpeechAmplitudeQ15>)] {
        &self.controls
    }
    pub fn total_frames(&self) -> u64 {
        self.render.initial().total_frames()
    }
}

/// Explicit offline authoring entrance. It does not promote a parser hypothesis
/// or attest an authenticated author. Only the matching immutable PCM projection
/// can be installed; raw Native declaration bytes are not an admitted receipt.
pub struct PreparedOfflinePcm {
    source: FrozenCarriedFormant,
    admission: SpeechOfflinePcmAdmission,
}
impl PreparedOfflinePcm {
    pub fn prepare(
        source: FrozenCarriedFormant,
        declaration: SpeechOfflinePcmDeclaration,
    ) -> Result<Self, NativeBindingRefusal> {
        use conduit_plot::rust_binding::BoundedSequence;
        let digest = |bytes: &[u8; 32]| {
            SpeechCarrierDigest::new(BoundedSequence::try_from_iter(bytes.iter().copied())?)
        };
        let admission = SpeechOfflinePcmAdmission::new(
            digest(source.projection_identity())?,
            digest(source.source_identity())?,
            declaration,
        )?;
        Ok(Self { source, admission })
    }
    pub fn source(&self) -> &FrozenCarriedFormant {
        &self.source
    }
    pub fn admission(&self) -> &SpeechOfflinePcmAdmission {
        &self.admission
    }
}
