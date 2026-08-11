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
            Self::RerouteCord => "reroute-cord",
            Self::ConfigureGear => "configure-gear",
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
            "reroute-cord" => Ok(Self::RerouteCord),
            "configure-gear" => Ok(Self::ConfigureGear),
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

    pub fn edit(
        request_id: PatchbayInteractionRequestId,
        edit: PatchbayEdit,
    ) -> Result<Self, InteractionError> {
        let (request_id, edit) = super::edit::request_id_and_edit(request_id, edit)?;
        Ok(Self::Edit { request_id, edit })
    }

    pub fn request_id(&self) -> &PatchbayInteractionRequestId {
        match self {
            Self::Select { request_id, .. }
            | Self::Invoke { request_id, .. }
            | Self::Edit { request_id, .. } => request_id,
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
            Self::Edit { request_id, edit } => {
                encoded.push(3);
                push_field(&mut encoded, request_id.as_str())?;
                encode_edit(&mut encoded, edit)?;
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
        match encoded[1] {
            1 => {
                let request_id =
                    PatchbayInteractionRequestId::new(read_field(encoded, &mut cursor)?)?;
                let second = read_field(encoded, &mut cursor)?;
                let third = read_field(encoded, &mut cursor)?;
                require_end(encoded, cursor)?;
                Ok(Self::Select {
                    request_id,
                    expanded_form_id: ExpandedFormId::from(second),
                    subject_identity: third,
                })
            }
            2 => {
                let request_id =
                    PatchbayInteractionRequestId::new(read_field(encoded, &mut cursor)?)?;
                let second = read_field(encoded, &mut cursor)?;
                let third = read_field(encoded, &mut cursor)?;
                require_end(encoded, cursor)?;
                Ok(Self::Invoke {
                    request_id,
                    invocation: PatchbayInvocation {
                        action: PatchbayAction::parse(&second)?,
                        target_identity: third,
                    },
                })
            }
            3 => {
                let request_id =
                    PatchbayInteractionRequestId::new(read_field(encoded, &mut cursor)?)?;
                let edit = decode_edit(encoded, &mut cursor)?;
                require_end(encoded, cursor)?;
                Self::edit(request_id, edit)
            }
            _ => Err(InteractionError::MalformedValue),
        }
    }
}

fn encode_edit(output: &mut Vec<u8>, edit: &PatchbayEdit) -> Result<(), InteractionError> {
    edit.validate()?;
    push_field(output, edit.operation())?;
    let basis = edit.basis();
    push_field(output, basis.source_document_id.as_str())?;
    output.extend_from_slice(&basis.source_revision.to_le_bytes());
    push_field(output, basis.expanded_form_id.as_str())?;
    match edit {
        PatchbayEdit::PlaceGear { kind_id, .. } => push_field(output, kind_id),
        PatchbayEdit::DuplicateGear {
            subject_identity, ..
        }
        | PatchbayEdit::RemoveGear {
            subject_identity, ..
        }
        | PatchbayEdit::RemoveCord {
            subject_identity, ..
        } => push_field(output, subject_identity),
        PatchbayEdit::ConnectPorts {
            source_identity,
            sink_identity,
            ..
        } => {
            push_field(output, source_identity)?;
            push_field(output, sink_identity)
        }
        PatchbayEdit::RerouteCord {
            cord_identity,
            endpoint_identity,
            ..
        } => {
            push_field(output, cord_identity)?;
            push_field(output, endpoint_identity)
        }
        PatchbayEdit::ConfigureGear {
            subject_identity,
            key,
            value,
            ..
        } => {
            push_field(output, subject_identity)?;
            push_field(output, key)?;
            encode_configuration_value(output, value)
        }
    }
}

fn decode_edit(input: &[u8], cursor: &mut usize) -> Result<PatchbayEdit, InteractionError> {
    let operation = read_field(input, cursor)?;
    let source_document_id = SourceDocumentId::from(read_field(input, cursor)?);
    let source_revision = read_u64(input, cursor)?;
    let expanded_form_id = ExpandedFormId::from(read_field(input, cursor)?);
    let basis = PatchbayEditBasis::new(source_document_id, source_revision, expanded_form_id)?;
    match operation.as_str() {
        "place-gear" => Ok(PatchbayEdit::PlaceGear {
            basis,
            kind_id: read_field(input, cursor)?,
        }),
        "duplicate-gear" => Ok(PatchbayEdit::DuplicateGear {
            basis,
            subject_identity: read_field(input, cursor)?,
        }),
        "remove-gear" => Ok(PatchbayEdit::RemoveGear {
            basis,
            subject_identity: read_field(input, cursor)?,
        }),
        "remove-cord" => Ok(PatchbayEdit::RemoveCord {
            basis,
            subject_identity: read_field(input, cursor)?,
        }),
        "connect-ports" => Ok(PatchbayEdit::ConnectPorts {
            basis,
            source_identity: read_field(input, cursor)?,
            sink_identity: read_field(input, cursor)?,
        }),
        "reroute-cord" => Ok(PatchbayEdit::RerouteCord {
            basis,
            cord_identity: read_field(input, cursor)?,
            endpoint_identity: read_field(input, cursor)?,
        }),
        "configure-gear" => Ok(PatchbayEdit::ConfigureGear {
            basis,
            subject_identity: read_field(input, cursor)?,
            key: read_field(input, cursor)?,
            value: decode_configuration_value(input, cursor)?,
        }),
        _ => Err(InteractionError::MalformedValue),
    }
}

fn encode_configuration_value(
    output: &mut Vec<u8>,
    value: &ConfigurationValue,
) -> Result<(), InteractionError> {
    match value {
        ConfigurationValue::Bool(value) => {
            output.push(1);
            output.push(u8::from(*value));
        }
        ConfigurationValue::U64(value) => {
            output.push(2);
            output.extend_from_slice(&value.to_le_bytes());
        }
        ConfigurationValue::I64(value) => {
            output.push(3);
            output.extend_from_slice(&value.to_le_bytes());
        }
        ConfigurationValue::Text(value) => {
            output.push(4);
            push_field(output, value)?;
        }
    }
    Ok(())
}

fn decode_configuration_value(
    input: &[u8],
    cursor: &mut usize,
) -> Result<ConfigurationValue, InteractionError> {
    let tag = *input.get(*cursor).ok_or(InteractionError::MalformedValue)?;
    *cursor = cursor
        .checked_add(1)
        .ok_or(InteractionError::MalformedValue)?;
    match tag {
        1 => {
            let value = *input.get(*cursor).ok_or(InteractionError::MalformedValue)?;
            *cursor = cursor
                .checked_add(1)
                .ok_or(InteractionError::MalformedValue)?;
            match value {
                0 => Ok(ConfigurationValue::Bool(false)),
                1 => Ok(ConfigurationValue::Bool(true)),
                _ => Err(InteractionError::MalformedValue),
            }
        }
        2 => Ok(ConfigurationValue::U64(read_u64(input, cursor)?)),
        3 => Ok(ConfigurationValue::I64(read_i64(input, cursor)?)),
        4 => Ok(ConfigurationValue::Text(read_field(input, cursor)?)),
        _ => Err(InteractionError::MalformedValue),
    }
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64, InteractionError> {
    let end = cursor
        .checked_add(8)
        .ok_or(InteractionError::MalformedValue)?;
    let bytes = input
        .get(*cursor..end)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .ok_or(InteractionError::MalformedValue)?;
    *cursor = end;
    Ok(u64::from_le_bytes(bytes))
}

fn read_i64(input: &[u8], cursor: &mut usize) -> Result<i64, InteractionError> {
    let end = cursor
        .checked_add(8)
        .ok_or(InteractionError::MalformedValue)?;
    let bytes = input
        .get(*cursor..end)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .ok_or(InteractionError::MalformedValue)?;
    *cursor = end;
    Ok(i64::from_le_bytes(bytes))
}

fn require_end(input: &[u8], cursor: usize) -> Result<(), InteractionError> {
    (cursor == input.len())
        .then_some(())
        .ok_or(InteractionError::MalformedValue)
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
