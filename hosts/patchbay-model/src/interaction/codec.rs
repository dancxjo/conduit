//! Finite transport encoding for the platform-neutral interaction value.

use super::*;

impl PatchbayAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenBack => "open-back",
            Self::Save => "save",
            Self::ToggleLinearView => "toggle-linear-view",
            Self::Birth => "birth",
            Self::Wake => "wake",
            Self::Lull => "lull",
            Self::Plan => "plan",
            Self::Play => "play",
            Self::Stop => "stop",
            Self::Hold => "hold",
            Self::PlaceGear => "place-gear",
            Self::DuplicateGear => "duplicate-gear",
            Self::RemoveGear => "remove-gear",
            Self::RemoveCord => "remove-cord",
            Self::ConnectPorts => "connect-ports",
        }
    }

    pub(super) fn parse(value: &str) -> Result<Self, InteractionError> {
        match value {
            "open-back" => Ok(Self::OpenBack),
            "save" => Ok(Self::Save),
            "toggle-linear-view" => Ok(Self::ToggleLinearView),
            "birth" => Ok(Self::Birth),
            "wake" => Ok(Self::Wake),
            "lull" => Ok(Self::Lull),
            "plan" => Ok(Self::Plan),
            "play" => Ok(Self::Play),
            "stop" => Ok(Self::Stop),
            "hold" => Ok(Self::Hold),
            "place-gear" => Ok(Self::PlaceGear),
            "duplicate-gear" => Ok(Self::DuplicateGear),
            "remove-gear" => Ok(Self::RemoveGear),
            "remove-cord" => Ok(Self::RemoveCord),
            "connect-ports" => Ok(Self::ConnectPorts),
            _ => Err(InteractionError::MalformedValue),
        }
    }
}

impl PatchbayInteractionRequest {
    pub fn select(
        request_id: PatchbayInteractionRequestId,
        subject: &crate::PatchbaySubjectRef,
    ) -> Result<Self, InteractionError> {
        validate_field(&subject.subject_identity)?;
        Ok(Self::Select {
            request_id,
            expanded_form_id: subject.expanded_form_id.clone(),
            subject_identity: subject.subject_identity.clone(),
        })
    }

    pub fn invoke(
        request_id: PatchbayInteractionRequestId,
        action: PatchbayAction,
        target_identity: impl Into<String>,
    ) -> Result<Self, InteractionError> {
        let target_identity = target_identity.into();
        validate_field(&target_identity)?;
        Ok(Self::Invoke {
            request_id,
            invocation: PatchbayInvocation {
                action,
                target_identity,
            },
        })
    }

    pub fn request_id(&self) -> &PatchbayInteractionRequestId {
        match self {
            Self::Select { request_id, .. } | Self::Invoke { request_id, .. } => request_id,
        }
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, InteractionError> {
        let mut encoded = Vec::with_capacity(MAX_INTERACTION_VALUE_BYTES as usize);
        encoded.push(1);
        match self {
            Self::Select {
                request_id,
                expanded_form_id,
                subject_identity,
            } => {
                encoded.push(1);
                push_field(&mut encoded, request_id.as_str())?;
                push_field(&mut encoded, expanded_form_id.as_str())?;
                push_field(&mut encoded, subject_identity)?;
            }
            Self::Invoke {
                request_id,
                invocation,
            } => {
                encoded.push(2);
                push_field(&mut encoded, request_id.as_str())?;
                push_field(&mut encoded, invocation.action.as_str())?;
                push_field(&mut encoded, &invocation.target_identity)?;
            }
        }
        if encoded.len() > MAX_INTERACTION_VALUE_BYTES as usize {
            return Err(InteractionError::ValueTooLarge);
        }
        Ok(encoded)
    }

    pub(super) fn decode(encoded: &[u8]) -> Result<Self, InteractionError> {
        if encoded.len() > MAX_INTERACTION_VALUE_BYTES as usize
            || encoded.len() < 2
            || encoded[0] != 1
        {
            return Err(InteractionError::MalformedValue);
        }
        let mut cursor = 2;
        let request_id = PatchbayInteractionRequestId::new(read_field(encoded, &mut cursor)?)?;
        let second = read_field(encoded, &mut cursor)?;
        let third = read_field(encoded, &mut cursor)?;
        if cursor != encoded.len() {
            return Err(InteractionError::MalformedValue);
        }
        match encoded[1] {
            1 => Ok(Self::Select {
                request_id,
                expanded_form_id: ExpandedFormId::from(second),
                subject_identity: third,
            }),
            2 => Ok(Self::Invoke {
                request_id,
                invocation: PatchbayInvocation {
                    action: PatchbayAction::parse(&second)?,
                    target_identity: third,
                },
            }),
            _ => Err(InteractionError::MalformedValue),
        }
    }
}

pub(super) fn validate_field(value: &str) -> Result<(), InteractionError> {
    if value.is_empty() || value.len() > MAX_INTERACTION_ID_BYTES {
        Err(InteractionError::InvalidIdentity)
    } else {
        Ok(())
    }
}

fn push_field(output: &mut Vec<u8>, value: &str) -> Result<(), InteractionError> {
    validate_field(value)?;
    let length = u16::try_from(value.len()).map_err(|_| InteractionError::ValueTooLarge)?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_field(input: &[u8], cursor: &mut usize) -> Result<String, InteractionError> {
    let end = cursor
        .checked_add(2)
        .ok_or(InteractionError::MalformedValue)?;
    let length = input
        .get(*cursor..end)
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .map(u16::from_le_bytes)
        .ok_or(InteractionError::MalformedValue)? as usize;
    *cursor = end;
    let end = cursor
        .checked_add(length)
        .ok_or(InteractionError::MalformedValue)?;
    let value = std::str::from_utf8(
        input
            .get(*cursor..end)
            .ok_or(InteractionError::MalformedValue)?,
    )
    .map_err(|_| InteractionError::MalformedValue)?;
    *cursor = end;
    validate_field(value)?;
    Ok(value.into())
}
