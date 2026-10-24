//! One validator over contiguous bytes or borrowed standard record payloads.
use crate::*;

pub(crate) fn decode(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    requirements: AssignedPlanRequirements<'_>,
    profile: Option<AssignedComputationProfile<'_>>,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    validate(
        bytes,
        bytes.len(),
        None,
        AssignedPlanRecords::payload_records(bytes),
        maxima,
        requirements,
        profile,
        None,
    )
}

pub(crate) fn decode_fore(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    required: AssignedPlanRequirements<'_>,
    computations: AssignedComputationProfile<'_>,
    fore: &[AssignedForeRequirement],
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    validate(
        bytes,
        bytes.len(),
        None,
        AssignedPlanRecords::payload_records(bytes),
        maxima,
        required,
        Some(computations),
        Some(fore),
    )
}

/// Validate existing canonical record payloads without copying them together.
/// Header is exactly the standard 125-byte assigned envelope. Logical length,
/// canonical record framing/digest, inventory and all existing checks remain
/// enforced. Borrowed records establish no Source provenance or authority.
/// Runtime must still pin graph/configuration and admit storage before Play.
pub fn decode_assigned_records_with_computations(
    header: &[u8],
    records: &[AssignedRecord<'_>],
    maxima: AssignedPlanMaxima,
    requirements: AssignedPlanRequirements<'_>,
    profile: AssignedComputationProfile<'_>,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    decode_records(header, records, maxima, requirements, profile, None)
}
pub(crate) fn decode_records(
    header: &[u8],
    records: &[AssignedRecord<'_>],
    maxima: AssignedPlanMaxima,
    requirements: AssignedPlanRequirements<'_>,
    profile: AssignedComputationProfile<'_>,
    fore: Option<&[AssignedForeRequirement]>,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    if header.len() != ASSIGNED_PLAN_HEADER_BYTES {
        return Err(AssignedPlanRefusal::WrongLength);
    }
    if records.len() > ASSIGNED_PLAN_COUNT_KINDS * usize::from(u8::MAX) {
        return Err(AssignedPlanRefusal::ExtraRecords);
    }
    let mut length = ASSIGNED_PLAN_HEADER_BYTES;
    let mut digest = sha256::State::new();
    for record in records {
        let size = u16::try_from(record.payload.len())
            .map_err(|_| AssignedPlanRefusal::MalformedRecord)?;
        length = length
            .checked_add(3 + record.payload.len())
            .ok_or(AssignedPlanRefusal::EncodedCapacityExceeded)?;
        if length > usize::from(maxima.encoded_bytes) {
            return Err(AssignedPlanRefusal::EncodedCapacityExceeded);
        }
        digest.update(&[record.tag]);
        digest.update(&size.to_le_bytes());
        digest.update(record.payload);
    }
    validate(
        header,
        length,
        Some(digest.finish()),
        records.iter().copied().map(Ok),
        maxima,
        requirements,
        Some(profile),
        fore,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate<'a>(
    bytes: &[u8],
    logical_length: usize,
    digest: Option<[u8; 32]>,
    records: impl Iterator<Item = Result<AssignedRecord<'a>, AssignedPlanRefusal>>,
    maxima: AssignedPlanMaxima,
    requirements: AssignedPlanRequirements<'_>,
    profile: Option<AssignedComputationProfile<'_>>,
    fore: Option<&[AssignedForeRequirement]>,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    let mut fore_validation = crate::fore::Validation::new(fore.unwrap_or(&[]))?;
    let mut computations = computation::Validation::new(profile)?;
    if bytes.len() < ASSIGNED_PLAN_HEADER_BYTES {
        return Err(AssignedPlanRefusal::WrongLength);
    }
    if &bytes[..8] != MAGIC {
        return Err(AssignedPlanRefusal::WrongMagic);
    }
    let schema = read_u16(bytes, 8)?;
    if fore.is_some()
        && (schema != ASSIGNED_FORE_NATIVE_PLAN_SCHEMA || !requirements.remote_bindings.is_empty())
    {
        return Err(AssignedPlanRefusal::WrongSchema);
    }
    if schema != ASSIGNED_PLAN_SCHEMA
        && !(schema == ASSIGNED_FORE_NATIVE_PLAN_SCHEMA && fore.is_some() && profile.is_some())
        && !(profile.is_some()
            && matches!(
                schema,
                ASSIGNED_KERNEL_PLAN_SCHEMA | ASSIGNED_NATIVE_PLAN_SCHEMA
            ))
    {
        return Err(AssignedPlanRefusal::WrongSchema);
    }
    let encoded_bytes = read_u16(bytes, 10)?;
    if usize::from(encoded_bytes) != logical_length {
        return Err(AssignedPlanRefusal::WrongLength);
    }
    if encoded_bytes > maxima.encoded_bytes {
        return Err(AssignedPlanRefusal::EncodedCapacityExceeded);
    }
    let runtime_state_bytes = read_u16(bytes, 12)?;
    if runtime_state_bytes > maxima.runtime_state_bytes {
        return Err(AssignedPlanRefusal::RuntimeCapacityExceeded);
    }
    let plan = read_identity(bytes, 16)?;
    let fragment = read_identity(bytes, 32)?;
    let host = read_identity(bytes, 48)?;
    let boot = read_identity(bytes, 64)?;
    if host != requirements.host {
        return Err(AssignedPlanRefusal::WrongHost);
    }
    if boot != requirements.boot {
        return Err(AssignedPlanRefusal::WrongBoot);
    }
    let mut counts = [0; ASSIGNED_PLAN_COUNT_KINDS];
    counts.copy_from_slice(&bytes[80..80 + ASSIGNED_PLAN_COUNT_KINDS]);
    for (index, (actual, maximum)) in counts.iter().zip(maxima.counts).enumerate() {
        if *actual > maximum {
            return Err(AssignedPlanRefusal::CountCapacityExceeded(index as u8 + 1));
        }
    }
    let digest_start = 80 + ASSIGNED_PLAN_COUNT_KINDS;
    let digest_end = digest_start + 32;
    let expected_digest = &bytes[digest_start..digest_end];
    let actual_digest = digest.unwrap_or_else(|| sha256::digest(&bytes[digest_end..]));
    if actual_digest != expected_digest {
        return Err(AssignedPlanRefusal::DigestMismatch);
    }

    let mut seen = [0_u8; ASSIGNED_PLAN_COUNT_KINDS];
    let mut host_calls = [false; 32];
    let mut resources = [false; 32];
    let mut remotes = [false; 16];
    if requirements.host_calls.len() > host_calls.len()
        || requirements.resources.len() > resources.len()
        || requirements.remote_bindings.len() > remotes.len()
    {
        return Err(AssignedPlanRefusal::MalformedRecord);
    }
    for record in records {
        let AssignedRecord { tag, payload } = record?;
        let length = payload.len();
        let valid_length = match tag {
            ASSIGNED_NODE => length == 52,
            ASSIGNED_PORT => length == 37,
            ASSIGNED_CONFIGURATION => {
                length == 27 || (20..=147).contains(&length) || (profile.is_some() && length >= 20)
            }
            ASSIGNED_CORD => {
                length
                    == if schema == ASSIGNED_PLAN_SCHEMA {
                        37
                    } else {
                        42
                    }
            }
            ASSIGNED_ROUTE => length == 8,
            ASSIGNED_ROUTE_TARGET => length == 6 || length == 7,
            ASSIGNED_HOST_CALL => {
                if matches!(
                    schema,
                    ASSIGNED_NATIVE_PLAN_SCHEMA | ASSIGNED_FORE_NATIVE_PLAN_SCHEMA
                ) {
                    length == 47 || length == ASSIGNED_NATIVE_CALL_BYTES
                } else {
                    length == 46
                }
            }
            ASSIGNED_RESOURCE => length == 8,
            ASSIGNED_SIGN => length == 37,
            ASSIGNED_REMOTE_ENDPOINT => {
                if schema == ASSIGNED_FORE_NATIVE_PLAN_SCHEMA {
                    length == ASSIGNED_LOCAL_FORE_BYTES
                } else {
                    length == 250
                }
            }
            ASSIGNED_STARTUP => length == 3 || length == 5,
            ASSIGNED_TERMINAL => length == 32,
            ASSIGNED_RESOURCE_CORD => length == 12,
            _ => false,
        };
        if !valid_length {
            return Err(AssignedPlanRefusal::MalformedRecord);
        }
        let valid_discriminants = match tag {
            ASSIGNED_PORT => payload[4] <= 1,
            ASSIGNED_CONFIGURATION => match payload[18] {
                0..=2 => length == 27,
                3 => computations.text(payload)?,
                _ => false,
            },
            ASSIGNED_CORD => {
                payload[18] <= 1
                    && payload[23] <= 1
                    && if schema == ASSIGNED_PLAN_SCHEMA {
                        payload[36] <= 2
                    } else {
                        payload[36] <= 3 && payload[41] <= 1
                    }
            }
            ASSIGNED_ROUTE_TARGET => length == 6 || payload[2] == 1,
            ASSIGNED_SIGN => payload[34] <= 3,
            ASSIGNED_RESOURCE_CORD => payload[10] <= 1 && payload[11] <= 2,
            ASSIGNED_REMOTE_ENDPOINT if schema == ASSIGNED_FORE_NATIVE_PLAN_SCHEMA => {
                payload[4] <= 1
            }
            ASSIGNED_REMOTE_ENDPOINT => {
                payload[228] <= 1
                    && payload[229] <= 4
                    && payload[230] <= 2
                    && payload[231] <= 2
                    && payload[232] <= 1
                    && payload[233] <= 1
                    && payload[234] <= 1
                    && payload[235] <= 3
            }
            ASSIGNED_HOST_CALL
                if matches!(
                    schema,
                    ASSIGNED_NATIVE_PLAN_SCHEMA | ASSIGNED_FORE_NATIVE_PLAN_SCHEMA
                ) =>
            {
                assigned_native_binding(payload).is_ok()
            }
            ASSIGNED_STARTUP => {
                (payload[0] == 0 && length == 5) || (payload[0] == 1 && length == 3)
            }
            _ => true,
        };
        if !valid_discriminants {
            return Err(AssignedPlanRefusal::MalformedRecord);
        }
        let count = &mut seen[usize::from(tag - 1)];
        *count = count
            .checked_add(1)
            .ok_or(AssignedPlanRefusal::ExtraRecords)?;
        match tag {
            ASSIGNED_HOST_CALL => {
                let identity = read_identity(payload, 4)?;
                let index = requirements
                    .host_calls
                    .iter()
                    .enumerate()
                    .find(|(index, required)| !host_calls[*index] && **required == identity)
                    .map(|(index, _)| index)
                    .ok_or(AssignedPlanRefusal::UnknownHostCall)?;
                host_calls[index] = true;
            }
            ASSIGNED_RESOURCE => {
                let resource = read_u16(payload, 2)?;
                let index = requirements
                    .resources
                    .iter()
                    .enumerate()
                    .find(|(index, required)| !resources[*index] && **required == resource)
                    .map(|(index, _)| index)
                    .ok_or(AssignedPlanRefusal::UnknownResource)?;
                resources[index] = true;
            }
            ASSIGNED_RESOURCE_CORD => {
                let resource = read_u16(payload, 8)?;
                if !requirements.resources.contains(&resource) {
                    return Err(AssignedPlanRefusal::UnknownResource);
                }
            }
            ASSIGNED_REMOTE_ENDPOINT if schema == ASSIGNED_FORE_NATIVE_PLAN_SCHEMA => {
                fore_validation.record(payload)?;
            }
            ASSIGNED_REMOTE_ENDPOINT => {
                let binding = AssignedRemoteBinding {
                    line: read_identity(payload, 0)?,
                    local_host: read_identity(payload, 16)?,
                    local_boot: read_identity(payload, 32)?,
                    peer_host: read_identity(payload, 48)?,
                    peer_boot: read_identity(payload, 64)?,
                };
                let index = requirements
                    .remote_bindings
                    .iter()
                    .enumerate()
                    .find(|(index, required)| !remotes[*index] && **required == binding)
                    .map(|(index, _)| index)
                    .ok_or(AssignedPlanRefusal::StaleRemoteEndpoint)?;
                remotes[index] = true;
            }
            _ => {}
        }
    }
    computations.finish()?;
    fore_validation.finish()?;
    if seen != counts {
        return Err(AssignedPlanRefusal::ExtraRecords);
    }
    if host_calls[..requirements.host_calls.len()]
        .iter()
        .any(|seen| !seen)
    {
        return Err(AssignedPlanRefusal::MissingHostCall);
    }
    if resources[..requirements.resources.len()]
        .iter()
        .any(|seen| !seen)
    {
        return Err(AssignedPlanRefusal::MissingResource);
    }
    if remotes[..requirements.remote_bindings.len()]
        .iter()
        .any(|seen| !seen)
    {
        return Err(AssignedPlanRefusal::MissingRemoteEndpoint);
    }
    Ok(AssignedPlanView {
        plan,
        fragment,
        host,
        boot,
        encoded_bytes,
        runtime_state_bytes,
        counts,
    })
}
