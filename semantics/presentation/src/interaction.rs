//! Typed, bounded human submission against one exact Presentation Manifestation.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    identity::hash_string, presentation::validate_id, Manifestation, ManifestationLifecycle,
    Presentation, PresentationActionRefusal, PresentationError,
};

pub const PRESENTATION_INTERACTION_VALUE_KIND: &str = "presentation/interaction@1";
pub const UTF8_TEXT_VALUE_KIND: &str = "value/text@1";
pub const MAX_PRESENTATION_INPUTS: usize = 64;
pub const MAX_PRESENTATION_INPUT_VALUE_BYTES: u32 = 4_096;
pub const MAX_PRESENTATION_INTERACTION_BYTES: usize = 8_192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationInput {
    pub identity: String,
    pub target: String,
    pub value_kind: String,
    pub maximum_bytes: u32,
    pub allow_empty: bool,
    pub label: String,
    pub accessibility_name: String,
    pub submit_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PresentationInteractionId(String);

impl PresentationInteractionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationInteraction {
    pub identity: PresentationInteractionId,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub manifestation_id: String,
    pub input_id: String,
    pub action_id: String,
    pub target: String,
    pub value_kind: String,
    pub value: Vec<u8>,
    pub sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationInteractionRefusal {
    InvalidPresentation,
    StalePresentation,
    StaleManifestation,
    FailedManifestation,
    UnknownInput,
    UnknownAction,
    WrongTarget,
    UnavailableAction,
    RefusedAction,
    WrongValueKind,
    EmptyValue,
    OversizeValue,
    MalformedEncoding,
    DuplicateDelivery,
    QueuePressure,
    EvidenceExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationInteractionFailure {
    Cancelled,
    AdapterUnavailable,
    DeliveryFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationInteractionDisposition {
    Accepted { operation_request_id: String },
    Refused(PresentationInteractionRefusal),
    Failed(PresentationInteractionFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationInteractionEvidence {
    pub interaction_id: PresentationInteractionId,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub manifestation_id: String,
    pub input_id: String,
    pub action_id: String,
    pub target: String,
    pub value_kind: String,
    pub value_bytes: u32,
    pub sequence: u64,
    pub disposition: PresentationInteractionDisposition,
}

impl Presentation {
    pub(crate) fn validate_inputs(&self) -> Result<(), PresentationError> {
        if self.inputs.len() > MAX_PRESENTATION_INPUTS {
            return Err(PresentationError::TooManyInputs);
        }
        for (index, input) in self.inputs.iter().enumerate() {
            for value in [
                &input.identity,
                &input.target,
                &input.value_kind,
                &input.submit_action,
            ] {
                validate_id(value)?;
            }
            crate::presentation::validate_text(&input.label)?;
            crate::presentation::validate_text(&input.accessibility_name)?;
            if input.maximum_bytes == 0 || input.maximum_bytes > MAX_PRESENTATION_INPUT_VALUE_BYTES
            {
                return Err(PresentationError::InvalidInputLimit);
            }
            if !self.has_subject(&input.target) {
                return Err(PresentationError::UnknownInputTarget);
            }
            let action = self
                .actions
                .iter()
                .find(|action| action.identity == input.submit_action)
                .ok_or(PresentationError::UnknownInputAction)?;
            if action.target != input.target {
                return Err(PresentationError::UnknownInputAction);
            }
            if self.inputs[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == input.identity)
            {
                return Err(PresentationError::DuplicateInput);
            }
        }
        Ok(())
    }

    pub(crate) fn inputs_len(&self) -> usize {
        self.inputs
            .iter()
            .map(|input| {
                input.identity.len()
                    + input.target.len()
                    + input.value_kind.len()
                    + input.label.len()
                    + input.accessibility_name.len()
                    + input.submit_action.len()
                    + 5
            })
            .sum()
    }

    pub(crate) fn hash_inputs(&self, digest: &mut Sha256) {
        for input in &self.inputs {
            hash_string(digest, &input.identity);
            hash_string(digest, &input.target);
            hash_string(digest, &input.value_kind);
            digest.update(input.maximum_bytes.to_le_bytes());
            digest.update([u8::from(input.allow_empty)]);
            hash_string(digest, &input.label);
            hash_string(digest, &input.accessibility_name);
            hash_string(digest, &input.submit_action);
        }
    }

    pub fn resolve_input(
        &self,
        revision: u64,
        identity: &str,
    ) -> Result<&PresentationInput, PresentationInteractionRefusal> {
        if revision != self.revision {
            return Err(PresentationInteractionRefusal::StalePresentation);
        }
        self.inputs
            .iter()
            .find(|input| input.identity == identity)
            .ok_or(PresentationInteractionRefusal::UnknownInput)
    }
}

impl PresentationInteraction {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        presentation: &Presentation,
        manifestation: &Manifestation,
        input_id: &str,
        action_id: &str,
        target: &str,
        value_kind: &str,
        value: &[u8],
        sequence: u64,
    ) -> Result<Self, PresentationInteractionRefusal> {
        presentation
            .validate()
            .map_err(|_| PresentationInteractionRefusal::InvalidPresentation)?;
        validate_manifestation(presentation, manifestation)?;
        let input = presentation.resolve_input(presentation.revision, input_id)?;
        let action = presentation
            .resolve_action(presentation.revision, action_id)
            .map_err(map_action_refusal)?;
        if input.submit_action != action.identity
            || input.target != target
            || action.target != target
        {
            return Err(PresentationInteractionRefusal::WrongTarget);
        }
        if input.value_kind != value_kind {
            return Err(PresentationInteractionRefusal::WrongValueKind);
        }
        if value.is_empty() && !input.allow_empty {
            return Err(PresentationInteractionRefusal::EmptyValue);
        }
        if value.len() > input.maximum_bytes as usize {
            return Err(PresentationInteractionRefusal::OversizeValue);
        }
        if value_kind == UTF8_TEXT_VALUE_KIND && core::str::from_utf8(value).is_err() {
            return Err(PresentationInteractionRefusal::MalformedEncoding);
        }
        let mut result = Self {
            identity: PresentationInteractionId(String::new()),
            presentation_id: presentation.identity.as_str().into(),
            presentation_revision: presentation.revision,
            manifestation_id: manifestation.manifestation_id.as_str().into(),
            input_id: input_id.into(),
            action_id: action_id.into(),
            target: target.into(),
            value_kind: value_kind.into(),
            value: value.into(),
            sequence,
        };
        result.identity = result.derived_identity();
        if result.encode().len() > MAX_PRESENTATION_INTERACTION_BYTES {
            return Err(PresentationInteractionRefusal::OversizeValue);
        }
        Ok(result)
    }

    pub fn validate_against(
        &self,
        presentation: &Presentation,
        manifestation: &Manifestation,
    ) -> Result<(), PresentationInteractionRefusal> {
        if self.presentation_id != presentation.identity.as_str()
            || self.presentation_revision != presentation.revision
        {
            return Err(PresentationInteractionRefusal::StalePresentation);
        }
        if self.manifestation_id != manifestation.manifestation_id.as_str() {
            return Err(PresentationInteractionRefusal::StaleManifestation);
        }
        let rebuilt = Self::new(
            presentation,
            manifestation,
            &self.input_id,
            &self.action_id,
            &self.target,
            &self.value_kind,
            &self.value,
            self.sequence,
        )?;
        if rebuilt.identity != self.identity {
            return Err(PresentationInteractionRefusal::MalformedEncoding);
        }
        Ok(())
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(MAX_PRESENTATION_INTERACTION_BYTES);
        bytes.extend_from_slice(b"CPI1");
        for field in [
            self.identity.as_str(),
            &self.presentation_id,
            &self.manifestation_id,
            &self.input_id,
            &self.action_id,
            &self.target,
            &self.value_kind,
        ] {
            push_field(&mut bytes, field.as_bytes());
        }
        bytes.extend_from_slice(&self.presentation_revision.to_le_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        push_field(&mut bytes, &self.value);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PresentationInteractionRefusal> {
        if bytes.len() > MAX_PRESENTATION_INTERACTION_BYTES || !bytes.starts_with(b"CPI1") {
            return Err(PresentationInteractionRefusal::MalformedEncoding);
        }
        let mut cursor = 4;
        let identity = read_text(bytes, &mut cursor)?;
        let presentation_id = read_text(bytes, &mut cursor)?;
        let manifestation_id = read_text(bytes, &mut cursor)?;
        let input_id = read_text(bytes, &mut cursor)?;
        let action_id = read_text(bytes, &mut cursor)?;
        let target = read_text(bytes, &mut cursor)?;
        let value_kind = read_text(bytes, &mut cursor)?;
        let presentation_revision = read_u64(bytes, &mut cursor)?;
        let sequence = read_u64(bytes, &mut cursor)?;
        let value = read_field(bytes, &mut cursor)?.to_vec();
        if cursor != bytes.len() {
            return Err(PresentationInteractionRefusal::MalformedEncoding);
        }
        Ok(Self {
            identity: PresentationInteractionId(identity),
            presentation_id,
            presentation_revision,
            manifestation_id,
            input_id,
            action_id,
            target,
            value_kind,
            value,
            sequence,
        })
    }

    fn derived_identity(&self) -> PresentationInteractionId {
        let mut digest = Sha256::new();
        digest.update(b"conduit.presentation/interaction@1\0");
        for field in [
            &self.presentation_id,
            &self.manifestation_id,
            &self.input_id,
            &self.action_id,
            &self.target,
            &self.value_kind,
        ] {
            hash_string(&mut digest, field);
        }
        digest.update(self.presentation_revision.to_le_bytes());
        digest.update(self.sequence.to_le_bytes());
        digest.update((self.value.len() as u32).to_le_bytes());
        digest.update(Sha256::digest(&self.value));
        let digest: [u8; 32] = digest.finalize().into();
        PresentationInteractionId(hex(&digest))
    }
}

pub(crate) fn linear_input(input: &PresentationInput) -> String {
    alloc::format!(
        "INPUT id={:?} target={:?} kind={:?} maximum_bytes={} allow_empty={} label={:?} accessibility={:?} submit_action={:?}",
        input.identity,
        input.target,
        input.value_kind,
        input.maximum_bytes,
        input.allow_empty,
        input.label,
        input.accessibility_name,
        input.submit_action,
    )
}

fn validate_manifestation(
    presentation: &Presentation,
    manifestation: &Manifestation,
) -> Result<(), PresentationInteractionRefusal> {
    if manifestation.presentation_id != presentation.identity
        || manifestation.presentation_revision != presentation.revision
    {
        return Err(PresentationInteractionRefusal::StaleManifestation);
    }
    match manifestation.lifecycle {
        ManifestationLifecycle::Available => Ok(()),
        ManifestationLifecycle::Failed => Err(PresentationInteractionRefusal::FailedManifestation),
        ManifestationLifecycle::Prepared
        | ManifestationLifecycle::Replaced
        | ManifestationLifecycle::Closed => Err(PresentationInteractionRefusal::StaleManifestation),
    }
}

fn map_action_refusal(value: PresentationActionRefusal) -> PresentationInteractionRefusal {
    match value {
        PresentationActionRefusal::StaleRevision => {
            PresentationInteractionRefusal::StalePresentation
        }
        PresentationActionRefusal::UnknownAction => PresentationInteractionRefusal::UnknownAction,
        PresentationActionRefusal::Unavailable { .. } => {
            PresentationInteractionRefusal::UnavailableAction
        }
        PresentationActionRefusal::Refused { .. } => PresentationInteractionRefusal::RefusedAction,
    }
}

fn push_field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}

fn read_field<'a>(
    input: &'a [u8],
    cursor: &mut usize,
) -> Result<&'a [u8], PresentationInteractionRefusal> {
    let length = read_u32(input, cursor)? as usize;
    let end = cursor
        .checked_add(length)
        .filter(|end| *end <= input.len())
        .ok_or(PresentationInteractionRefusal::MalformedEncoding)?;
    let value = &input[*cursor..end];
    *cursor = end;
    Ok(value)
}

fn read_text(input: &[u8], cursor: &mut usize) -> Result<String, PresentationInteractionRefusal> {
    core::str::from_utf8(read_field(input, cursor)?)
        .map(String::from)
        .map_err(|_| PresentationInteractionRefusal::MalformedEncoding)
}

fn read_u32(input: &[u8], cursor: &mut usize) -> Result<u32, PresentationInteractionRefusal> {
    let end = cursor
        .checked_add(4)
        .filter(|end| *end <= input.len())
        .ok_or(PresentationInteractionRefusal::MalformedEncoding)?;
    let value = u32::from_le_bytes(input[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64, PresentationInteractionRefusal> {
    let end = cursor
        .checked_add(8)
        .filter(|end| *end <= input.len())
        .ok_or(PresentationInteractionRefusal::MalformedEncoding)?;
    let value = u64::from_le_bytes(input[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
