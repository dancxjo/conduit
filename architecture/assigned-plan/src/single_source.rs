//! Bounded decoder for the generic one-source assigned-Plan profile.
//!
//! This consumes the same assigned-Plan schema as [`crate::decode_assigned_plan`].
//! It only admits the shape executed by [`conduit_kernel::SingleSourceExecutor`]:
//! one node, one output Port, one Host operation, no Cords or remote endpoints,
//! and an exact finite inventory supplied by planning.

use crate::{
    sha256, AssignedIdentity, AssignedPlanMaxima, AssignedPlanRefusal, AssignedPlanView,
    ASSIGNED_CONFIGURATION, ASSIGNED_CORD, ASSIGNED_HOST_OPERATION, ASSIGNED_NODE,
    ASSIGNED_PLAN_COUNT_KINDS, ASSIGNED_PLAN_HEADER_BYTES, ASSIGNED_PLAN_SCHEMA, ASSIGNED_PORT,
    ASSIGNED_REMOTE_ENDPOINT, ASSIGNED_RESOURCE, ASSIGNED_ROUTE, ASSIGNED_ROUTE_TARGET,
    ASSIGNED_SIGN, ASSIGNED_STARTUP, ASSIGNED_TERMINAL, MAGIC,
};

#[derive(Clone, Copy)]
pub struct AssignedSingleSourceRequirements<'a> {
    pub host: AssignedIdentity,
    pub boot: AssignedIdentity,
    pub counts: [u8; ASSIGNED_PLAN_COUNT_KINDS],
    pub operation: AssignedIdentity,
    pub resources: &'a [u16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedSingleSourceView {
    pub assigned: AssignedPlanView,
    pub maximum_step_work: u16,
    pub maximum_output_bytes: u32,
    pub output_port: u16,
}

pub fn decode_assigned_single_source(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    required: AssignedSingleSourceRequirements<'_>,
) -> Result<AssignedSingleSourceView, AssignedPlanRefusal> {
    if bytes.len() < ASSIGNED_PLAN_HEADER_BYTES {
        return Err(AssignedPlanRefusal::WrongLength);
    }
    if bytes.get(..8) != Some(MAGIC) {
        return Err(AssignedPlanRefusal::WrongMagic);
    }
    if u16_at(bytes, 8)? != ASSIGNED_PLAN_SCHEMA {
        return Err(AssignedPlanRefusal::WrongSchema);
    }
    let encoded_bytes = u16_at(bytes, 10)?;
    if usize::from(encoded_bytes) != bytes.len() {
        return Err(AssignedPlanRefusal::WrongLength);
    }
    if encoded_bytes > maxima.encoded_bytes {
        return Err(AssignedPlanRefusal::EncodedCapacityExceeded);
    }
    let runtime_state_bytes = u16_at(bytes, 12)?;
    if runtime_state_bytes > maxima.runtime_state_bytes {
        return Err(AssignedPlanRefusal::RuntimeCapacityExceeded);
    }
    let plan = identity_at(bytes, 16)?;
    let fragment = identity_at(bytes, 32)?;
    let host = identity_at(bytes, 48)?;
    let boot = identity_at(bytes, 64)?;
    if host != required.host {
        return Err(AssignedPlanRefusal::WrongHost);
    }
    if boot != required.boot {
        return Err(AssignedPlanRefusal::WrongBoot);
    }
    let mut counts = [0; ASSIGNED_PLAN_COUNT_KINDS];
    counts.copy_from_slice(&bytes[80..92]);
    if counts != required.counts {
        return Err(AssignedPlanRefusal::ExtraRecords);
    }
    let mut index = 0;
    while index < ASSIGNED_PLAN_COUNT_KINDS {
        if counts[index] > maxima.counts[index] {
            return Err(AssignedPlanRefusal::CountCapacityExceeded(index as u8 + 1));
        }
        index += 1;
    }
    if sha256::digest(&bytes[ASSIGNED_PLAN_HEADER_BYTES..]) != bytes[92..124] {
        return Err(AssignedPlanRefusal::DigestMismatch);
    }

    let mut seen = [0_u8; ASSIGNED_PLAN_COUNT_KINDS];
    let mut resources = [false; 8];
    if required.resources.len() > resources.len() {
        return Err(AssignedPlanRefusal::MalformedRecord);
    }
    let mut maximum_step_work = None;
    let mut maximum_output_bytes = None;
    let mut output_port = None;
    let mut operation_seen = false;
    let mut cursor = ASSIGNED_PLAN_HEADER_BYTES;
    while cursor < bytes.len() {
        let tag = *bytes
            .get(cursor)
            .ok_or(AssignedPlanRefusal::MalformedRecord)?;
        if tag == 0 || usize::from(tag) > ASSIGNED_PLAN_COUNT_KINDS {
            return Err(AssignedPlanRefusal::UnknownRecord(tag));
        }
        let length = usize::from(u16_at(bytes, cursor + 1)?);
        let start = cursor + 3;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or(AssignedPlanRefusal::MalformedRecord)?;
        let payload = &bytes[start..end];
        let valid = match tag {
            ASSIGNED_NODE => {
                length == 52
                    && u16_at(payload, 0)? == 0
                    && maximum_step_work.replace(u16_at(payload, 2)?).is_none()
            }
            ASSIGNED_PORT => {
                length == 37
                    && u16_at(payload, 0)? == 0
                    && payload[4] == 1
                    && output_port.replace(u16_at(payload, 2)?).is_none()
            }
            ASSIGNED_HOST_OPERATION => {
                let identity = identity_at(payload, 4)?;
                let unique = !operation_seen;
                operation_seen = true;
                length == 46
                    && unique
                    && u16_at(payload, 0)? == 0
                    && u16_at(payload, 2)? == 0
                    && identity == required.operation
                    && u16_at(payload, 36)? == 1
                    && u32_at(payload, 38)? == 0
                    && maximum_output_bytes.replace(u32_at(payload, 42)?).is_none()
            }
            ASSIGNED_RESOURCE => {
                if length != 8 {
                    false
                } else {
                    let resource = u16_at(payload, 2)?;
                    let mut found = None;
                    let mut resource_index = 0;
                    while resource_index < required.resources.len() {
                        if required.resources[resource_index] == resource
                            && !resources[resource_index]
                        {
                            found = Some(resource_index);
                            break;
                        }
                        resource_index += 1;
                    }
                    if let Some(found) = found {
                        resources[found] = true;
                        true
                    } else {
                        false
                    }
                }
            }
            ASSIGNED_SIGN => length == 37 && payload[34] <= 2,
            ASSIGNED_STARTUP => {
                (length == 5 && payload[0] == 0) || (length == 3 && payload[0] == 1)
            }
            ASSIGNED_TERMINAL => length == 32,
            ASSIGNED_CONFIGURATION
            | ASSIGNED_CORD
            | ASSIGNED_ROUTE
            | ASSIGNED_ROUTE_TARGET
            | ASSIGNED_REMOTE_ENDPOINT => false,
            _ => false,
        };
        if !valid {
            return Err(AssignedPlanRefusal::MalformedRecord);
        }
        let count = &mut seen[usize::from(tag - 1)];
        *count = count
            .checked_add(1)
            .ok_or(AssignedPlanRefusal::ExtraRecords)?;
        cursor = end;
    }
    if seen != counts || !operation_seen {
        return Err(AssignedPlanRefusal::ExtraRecords);
    }
    let mut resource_index = 0;
    while resource_index < required.resources.len() {
        if !resources[resource_index] {
            return Err(AssignedPlanRefusal::MissingResource);
        }
        resource_index += 1;
    }
    Ok(AssignedSingleSourceView {
        assigned: AssignedPlanView {
            plan,
            fragment,
            host,
            boot,
            encoded_bytes,
            runtime_state_bytes,
            counts,
        },
        maximum_step_work: maximum_step_work.ok_or(AssignedPlanRefusal::MissingOperation)?,
        maximum_output_bytes: maximum_output_bytes.ok_or(AssignedPlanRefusal::MissingOperation)?,
        output_port: output_port.ok_or(AssignedPlanRefusal::MalformedRecord)?,
    })
}

fn identity_at(bytes: &[u8], offset: usize) -> Result<AssignedIdentity, AssignedPlanRefusal> {
    let mut value = [0; 16];
    value.copy_from_slice(
        bytes
            .get(offset..offset + 16)
            .ok_or(AssignedPlanRefusal::MalformedRecord)?,
    );
    Ok(AssignedIdentity(value))
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, AssignedPlanRefusal> {
    let low = *bytes
        .get(offset)
        .ok_or(AssignedPlanRefusal::MalformedRecord)?;
    let high = *bytes
        .get(offset + 1)
        .ok_or(AssignedPlanRefusal::MalformedRecord)?;
    Ok(u16::from_le_bytes([low, high]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, AssignedPlanRefusal> {
    Ok(u32::from(u16_at(bytes, offset)?) | (u32::from(u16_at(bytes, offset + 2)?) << 16))
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use self::std::vec::Vec;

    const COUNTS: [u8; ASSIGNED_PLAN_COUNT_KINDS] = [1, 1, 0, 0, 0, 0, 1, 3, 4, 0, 1, 2];

    #[test]
    fn exact_single_source_profile_accepts_one_generic_plan_and_refuses_inventory_drift() {
        let operation = AssignedIdentity([7; 16]);
        let mut bytes = fixture(operation);
        let requirements = AssignedSingleSourceRequirements {
            host: AssignedIdentity([3; 16]),
            boot: AssignedIdentity([4; 16]),
            counts: COUNTS,
            operation,
            resources: &[0, 1, 2],
        };
        let decoded = decode_assigned_single_source(
            &bytes,
            AssignedPlanMaxima {
                encoded_bytes: bytes.len() as u16,
                runtime_state_bytes: 192,
                counts: COUNTS,
            },
            requirements,
        )
        .unwrap();
        assert_eq!(decoded.maximum_step_work, 3);
        assert_eq!(decoded.maximum_output_bytes, 1);
        assert_eq!(decoded.output_port, 0);

        let operation_offset = ASSIGNED_PLAN_HEADER_BYTES + 3 + 52 + 3 + 37 + 3 + 4;
        bytes[operation_offset] ^= 1;
        refresh_digest(&mut bytes);
        assert_eq!(
            decode_assigned_single_source(
                &bytes,
                AssignedPlanMaxima {
                    encoded_bytes: bytes.len() as u16,
                    runtime_state_bytes: 192,
                    counts: COUNTS,
                },
                requirements,
            ),
            Err(AssignedPlanRefusal::MalformedRecord)
        );
    }

    fn fixture(operation: AssignedIdentity) -> Vec<u8> {
        let mut records = Vec::new();
        let mut node = [0; 52];
        node[2..4].copy_from_slice(&3_u16.to_le_bytes());
        record(&mut records, ASSIGNED_NODE, &node);
        let mut port = [0; 37];
        port[4] = 1;
        record(&mut records, ASSIGNED_PORT, &port);
        let mut host_operation = [0; 46];
        host_operation[4..20].copy_from_slice(&operation.0);
        host_operation[36..38].copy_from_slice(&1_u16.to_le_bytes());
        host_operation[42..46].copy_from_slice(&1_u32.to_le_bytes());
        record(&mut records, ASSIGNED_HOST_OPERATION, &host_operation);
        for resource in 0_u16..3 {
            let mut payload = [0; 8];
            payload[2..4].copy_from_slice(&resource.to_le_bytes());
            record(&mut records, ASSIGNED_RESOURCE, &payload);
        }
        for _ in 0..4 {
            record(&mut records, ASSIGNED_SIGN, &[0; 37]);
        }
        record(&mut records, ASSIGNED_STARTUP, &[1, 0, 0]);
        record(&mut records, ASSIGNED_TERMINAL, &[0; 32]);
        record(&mut records, ASSIGNED_TERMINAL, &[0; 32]);

        let mut bytes = self::std::vec![0; ASSIGNED_PLAN_HEADER_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&ASSIGNED_PLAN_SCHEMA.to_le_bytes());
        bytes[12..14].copy_from_slice(&192_u16.to_le_bytes());
        bytes[16..32].copy_from_slice(&[1; 16]);
        bytes[32..48].copy_from_slice(&[2; 16]);
        bytes[48..64].copy_from_slice(&[3; 16]);
        bytes[64..80].copy_from_slice(&[4; 16]);
        bytes[80..92].copy_from_slice(&COUNTS);
        bytes.extend_from_slice(&records);
        let length = bytes.len() as u16;
        bytes[10..12].copy_from_slice(&length.to_le_bytes());
        refresh_digest(&mut bytes);
        bytes
    }

    fn record(bytes: &mut Vec<u8>, tag: u8, payload: &[u8]) {
        bytes.push(tag);
        bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        bytes.extend_from_slice(payload);
    }

    fn refresh_digest(bytes: &mut [u8]) {
        let digest = sha256::digest(&bytes[ASSIGNED_PLAN_HEADER_BYTES..]);
        bytes[92..124].copy_from_slice(&digest);
    }
}
