//! Typed, bounded human participation through one exact Face and Show.

use alloc::{string::String, vec::Vec};
use conduit_core::ValueConstraintRefusal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    identity::hash_string, presentation::validate_id, ManifestationLifecycle, MaskShow,
    Presentation, PresentationActionRefusal,
};

pub const FACE_INTERACTION_VALUE_KIND: &str = "face/interaction@2";
pub const UTF8_TEXT_VALUE_KIND: &str = "value/text";
pub const MAX_FACE_ACTION_ARGUMENTS: usize = 64;
pub const MAX_FACE_ACTION_ARGUMENT_BYTES: u32 = 4_096;
pub const MAX_FACE_INTERACTION_BYTES: usize = 8_192;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FaceInteractionId(String);

impl FaceInteractionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceInteractionArgument {
    pub name: String,
    pub value_kind: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceInteraction {
    pub identity: FaceInteractionId,
    pub face_id: String,
    pub face_revision: u64,
    pub show_id: String,
    pub action_id: String,
    pub target: String,
    pub arguments: Vec<FaceInteractionArgument>,
    pub sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceInteractionRefusal {
    InvalidFace,
    StaleFace,
    StaleShow,
    FailedShow,
    UnknownAction,
    NoQueuedInteraction,
    WrongTarget,
    UnavailableAction,
    RefusedAction,
    DuplicateArgument,
    MissingArgument,
    UnknownArgument,
    WrongValueKind,
    OversizeValue,
    MalformedEncoding,
    ViolatedConstraint,
    ValidatorIncapacity,
    DuplicateDelivery,
    QueuePressure,
    EvidenceExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceInteractionFailure {
    Cancelled,
    AdapterUnavailable,
    DeliveryFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceInteractionDisposition {
    Accepted { operation_request_id: String },
    Refused(FaceInteractionRefusal),
    Failed(FaceInteractionFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceInteractionArgumentEvidence {
    pub name: String,
    pub value_kind: String,
    pub value_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceInteractionEvidence {
    pub interaction_id: FaceInteractionId,
    pub face_id: String,
    pub face_revision: u64,
    pub show_id: String,
    pub action_id: String,
    pub target: String,
    pub arguments: Vec<FaceInteractionArgumentEvidence>,
    pub sequence: u64,
    pub disposition: FaceInteractionDisposition,
}

impl FaceInteraction {
    pub fn new(
        face: &Presentation,
        show: &MaskShow,
        action_id: &str,
        target: &str,
        arguments: Vec<FaceInteractionArgument>,
        sequence: u64,
    ) -> Result<Self, FaceInteractionRefusal> {
        face.validate()
            .map_err(|_| FaceInteractionRefusal::InvalidFace)?;
        validate_show(face, show)?;
        let action = face
            .resolve_action(face.revision, action_id)
            .map_err(map_action_refusal)?;
        if action.target != target {
            return Err(FaceInteractionRefusal::WrongTarget);
        }
        for (index, argument) in arguments.iter().enumerate() {
            validate_id(&argument.name).map_err(|_| FaceInteractionRefusal::UnknownArgument)?;
            if arguments[index + 1..]
                .iter()
                .any(|candidate| candidate.name == argument.name)
            {
                return Err(FaceInteractionRefusal::DuplicateArgument);
            }
        }
        let declarations = &action.arguments;
        for declaration in declarations {
            let argument = arguments
                .iter()
                .find(|argument| argument.name == declaration.name)
                .ok_or(FaceInteractionRefusal::MissingArgument)?;
            if declaration.contract.value_kind.as_str() != argument.value_kind {
                return Err(FaceInteractionRefusal::WrongValueKind);
            }
            declaration
                .contract
                .validate(&argument.value)
                .map_err(map_value_refusal)?;
        }
        if arguments.len() != declarations.len() {
            return Err(FaceInteractionRefusal::UnknownArgument);
        }
        let mut result = Self {
            identity: FaceInteractionId(String::new()),
            face_id: face.identity.as_str().into(),
            face_revision: face.revision,
            show_id: show.show_id.as_str().into(),
            action_id: action_id.into(),
            target: target.into(),
            arguments,
            sequence,
        };
        result.identity = result.derived_identity();
        if result.encode().len() > MAX_FACE_INTERACTION_BYTES {
            return Err(FaceInteractionRefusal::OversizeValue);
        }
        Ok(result)
    }

    pub fn validate_against(
        &self,
        face: &Presentation,
        show: &MaskShow,
    ) -> Result<(), FaceInteractionRefusal> {
        if self.face_id != face.identity.as_str() || self.face_revision != face.revision {
            return Err(FaceInteractionRefusal::StaleFace);
        }
        if self.show_id != show.show_id.as_str() {
            return Err(FaceInteractionRefusal::StaleShow);
        }
        let rebuilt = Self::new(
            face,
            show,
            &self.action_id,
            &self.target,
            self.arguments.clone(),
            self.sequence,
        )?;
        if rebuilt.identity != self.identity {
            return Err(FaceInteractionRefusal::MalformedEncoding);
        }
        Ok(())
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(MAX_FACE_INTERACTION_BYTES);
        bytes.extend_from_slice(b"CFI2");
        for field in [
            self.identity.as_str(),
            &self.face_id,
            &self.show_id,
            &self.action_id,
            &self.target,
        ] {
            push_field(&mut bytes, field.as_bytes());
        }
        bytes.extend_from_slice(&self.face_revision.to_le_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes.extend_from_slice(&(self.arguments.len() as u32).to_le_bytes());
        for argument in &self.arguments {
            push_field(&mut bytes, argument.name.as_bytes());
            push_field(&mut bytes, argument.value_kind.as_bytes());
            push_field(&mut bytes, &argument.value);
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FaceInteractionRefusal> {
        if bytes.len() > MAX_FACE_INTERACTION_BYTES || !bytes.starts_with(b"CFI2") {
            return Err(FaceInteractionRefusal::MalformedEncoding);
        }
        let mut cursor = 4;
        let identity = read_text(bytes, &mut cursor)?;
        let face_id = read_text(bytes, &mut cursor)?;
        let show_id = read_text(bytes, &mut cursor)?;
        let action_id = read_text(bytes, &mut cursor)?;
        let target = read_text(bytes, &mut cursor)?;
        let face_revision = read_u64(bytes, &mut cursor)?;
        let sequence = read_u64(bytes, &mut cursor)?;
        let argument_count = read_u32(bytes, &mut cursor)? as usize;
        if argument_count > MAX_FACE_ACTION_ARGUMENTS {
            return Err(FaceInteractionRefusal::MalformedEncoding);
        }
        let mut arguments = Vec::with_capacity(argument_count);
        for _ in 0..argument_count {
            arguments.push(FaceInteractionArgument {
                name: read_text(bytes, &mut cursor)?,
                value_kind: read_text(bytes, &mut cursor)?,
                value: read_field(bytes, &mut cursor)?.to_vec(),
            });
        }
        if cursor != bytes.len() {
            return Err(FaceInteractionRefusal::MalformedEncoding);
        }
        Ok(Self {
            identity: FaceInteractionId(identity),
            face_id,
            face_revision,
            show_id,
            action_id,
            target,
            arguments,
            sequence,
        })
    }

    fn derived_identity(&self) -> FaceInteractionId {
        let mut digest = Sha256::new();
        digest.update(b"conduit.face/interaction@2\0");
        for field in [&self.face_id, &self.show_id, &self.action_id, &self.target] {
            hash_string(&mut digest, field);
        }
        digest.update(self.face_revision.to_le_bytes());
        digest.update(self.sequence.to_le_bytes());
        digest.update((self.arguments.len() as u32).to_le_bytes());
        for argument in &self.arguments {
            hash_string(&mut digest, &argument.name);
            hash_string(&mut digest, &argument.value_kind);
            digest.update((argument.value.len() as u32).to_le_bytes());
            digest.update(Sha256::digest(&argument.value));
        }
        let digest: [u8; 32] = digest.finalize().into();
        FaceInteractionId(hex(&digest))
    }
}

fn map_value_refusal(refusal: ValueConstraintRefusal) -> FaceInteractionRefusal {
    match refusal {
        ValueConstraintRefusal::Oversize { .. } => FaceInteractionRefusal::OversizeValue,
        ValueConstraintRefusal::MalformedPrimitive(_) => FaceInteractionRefusal::MalformedEncoding,
        ValueConstraintRefusal::WrongConstraintKind => FaceInteractionRefusal::ValidatorIncapacity,
        ValueConstraintRefusal::ByteLength
        | ValueConstraintRefusal::UnsignedRange
        | ValueConstraintRefusal::SignedRange
        | ValueConstraintRefusal::FixedIntegerRange
        | ValueConstraintRefusal::QuantityRange
        | ValueConstraintRefusal::FloatFinite
        | ValueConstraintRefusal::FloatRange
        | ValueConstraintRefusal::Membership
        | ValueConstraintRefusal::TextPattern => FaceInteractionRefusal::ViolatedConstraint,
    }
}

fn validate_show(face: &Presentation, show: &MaskShow) -> Result<(), FaceInteractionRefusal> {
    show.validate(face)
        .map_err(|_| FaceInteractionRefusal::StaleShow)?;
    match show.show.lifecycle {
        ManifestationLifecycle::Available => Ok(()),
        ManifestationLifecycle::Failed => Err(FaceInteractionRefusal::FailedShow),
        ManifestationLifecycle::Prepared
        | ManifestationLifecycle::Replaced
        | ManifestationLifecycle::Closed => Err(FaceInteractionRefusal::StaleShow),
    }
}

fn map_action_refusal(value: PresentationActionRefusal) -> FaceInteractionRefusal {
    match value {
        PresentationActionRefusal::StaleRevision => FaceInteractionRefusal::StaleFace,
        PresentationActionRefusal::UnknownAction => FaceInteractionRefusal::UnknownAction,
        PresentationActionRefusal::Unavailable { .. } => FaceInteractionRefusal::UnavailableAction,
        PresentationActionRefusal::Refused { .. } => FaceInteractionRefusal::RefusedAction,
    }
}

fn push_field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}

fn read_field<'a>(input: &'a [u8], cursor: &mut usize) -> Result<&'a [u8], FaceInteractionRefusal> {
    let length = read_u32(input, cursor)? as usize;
    let end = cursor
        .checked_add(length)
        .filter(|end| *end <= input.len())
        .ok_or(FaceInteractionRefusal::MalformedEncoding)?;
    let value = &input[*cursor..end];
    *cursor = end;
    Ok(value)
}

fn read_text(input: &[u8], cursor: &mut usize) -> Result<String, FaceInteractionRefusal> {
    core::str::from_utf8(read_field(input, cursor)?)
        .map(String::from)
        .map_err(|_| FaceInteractionRefusal::MalformedEncoding)
}

fn read_u32(input: &[u8], cursor: &mut usize) -> Result<u32, FaceInteractionRefusal> {
    let end = cursor
        .checked_add(4)
        .filter(|end| *end <= input.len())
        .ok_or(FaceInteractionRefusal::MalformedEncoding)?;
    let value = u32::from_le_bytes(input[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64, FaceInteractionRefusal> {
    let end = cursor
        .checked_add(8)
        .filter(|end| *end <= input.len())
        .ok_or(FaceInteractionRefusal::MalformedEncoding)?;
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
