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
use conduit_audio::{
    AudioCumulativeProjectionReceipt, AudioSampleRateBasis, AudioTrajectoryAnchor,
};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum CarriedFormantRefusal {
    Native(NativeBindingRefusal),
    Realization(IntentRealizationRefusal),
    Render(RenderRefusal),
    UnsupportedPhoneFeatures { phone: usize },
    Grid(crate::intent_duration_grid::IntentDurationGridRefusal),
    Geometry { event: Option<usize> },
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
    sample_basis: AudioSampleRateBasis,
    grid: Vec<AudioCumulativeProjectionReceipt>,
    controls: Vec<(Option<SpeechCycleQ8AtRate>, Option<SpeechAmplitudeQ15>)>,
}
impl FrozenCarriedFormant {
    pub fn prepare(
        carrier: &PreparedSpeechCorrespondence<'_, '_>,
        profile: &SpeechFormantVoiceProfile,
        boundaries: &SpeechFormantBoundaryProfile,
        anchor: &AudioTrajectoryAnchor,
        basis: &AudioSampleRateBasis,
    ) -> Result<Self, CarriedFormantRefusal> {
        for (phone, token) in carrier.phones().tokens().as_slice().iter().enumerate() {
            if !token.features().get().as_slice().is_empty() {
                return Err(CarriedFormantRefusal::UnsupportedPhoneFeatures { phone });
            }
        }
        if *basis.sample_rate_hz() != u64::from(crate::SAMPLE_RATE_HZ) {
            return Err(CarriedFormantRefusal::Geometry { event: None });
        }
        let grid = crate::intent_duration_grid::prepare_intent_duration_grid(
            carrier.intent(),
            carrier.text(),
            anchor,
            Some(basis),
        )
        .map_err(CarriedFormantRefusal::Grid)?;
        let realized = prepare_intent_realization(
            carrier.intent(),
            carrier.inventory().inventory(),
            profile,
            boundaries,
        )
        .map_err(CarriedFormantRefusal::Realization)?;
        for (event, (span, step)) in realized
            .timing()
            .spans()
            .iter()
            .zip(grid.steps())
            .enumerate()
        {
            if span.cumulative_end().whole_frames() != step.integer_target().whole_frames()
                || span.frame_count() != step.result().raw().frame_count()
            {
                return Err(CarriedFormantRefusal::Geometry { event: Some(event) });
            }
        }
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
        let sample_basis = basis.clone();
        let bytes = sample_basis
            .clone()
            .encode()
            .map_err(CarriedFormantRefusal::Native)?;
        projection.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        projection.extend_from_slice(&bytes);
        let mut programs = Vec::new();
        for step in grid.steps() {
            for execution in step.executions() {
                let program = execution.source_program_hex();
                if !programs.contains(&program) {
                    programs.push(program);
                }
            }
        }
        projection.extend_from_slice(&(programs.len() as u64).to_le_bytes());
        for program in &programs {
            projection.extend_from_slice(&(program.len() as u64).to_le_bytes());
            projection.extend_from_slice(program.as_bytes());
        }
        for step in grid.steps() {
            for bytes in [step.original_canonical(), step.admitted_canonical()] {
                projection.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                projection.extend_from_slice(bytes);
            }
            projection.extend_from_slice(&(step.executions().len() as u64).to_le_bytes());
            for execution in step.executions() {
                let program_index = programs
                    .iter()
                    .position(|program| *program == execution.source_program_hex())
                    .expect("retained execution program is in the evidence table");
                projection.extend_from_slice(&(program_index as u64).to_le_bytes());
                for bytes in [execution.input_canonical(), execution.output_canonical()] {
                    projection.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                    projection.extend_from_slice(bytes);
                }
            }
        }
        let grid = grid.into_steps();
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
            sample_basis,
            grid,
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
    pub fn sample_basis(&self) -> &AudioSampleRateBasis {
        &self.sample_basis
    }
    pub fn sample_projection_receipts(&self) -> &[AudioCumulativeProjectionReceipt] {
        &self.grid
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
            SpeechCarrierDigest::new(
                BoundedSequence::try_from_iter(bytes.iter().copied())
                    .expect("exact 32-byte digest"),
            )
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
