//! Exact bounded structured Info carried by an ordinary connection envelope.

use conduit_core::{
    decode_structured_transport, encode_structured_transport, ConnectionEnvelope, ConnectionId,
    PlanId, StructuredInfoTransportRefusal, StructuredInfoType, StructuredInfoValue,
    PROTOCOL_VERSION,
};

use crate::WireError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredWireRefusal {
    Wire(WireError),
    Structured(StructuredInfoTransportRefusal),
    ProfileMismatch,
}

pub fn structured_connection_envelope(
    plan_id: PlanId,
    connection_id: ConnectionId,
    sequence: u64,
    value: &StructuredInfoValue,
    maximum_payload_bytes: u32,
) -> Result<ConnectionEnvelope, StructuredWireRefusal> {
    let profile = value.value_type().profile().map_err(|error| {
        StructuredWireRefusal::Structured(StructuredInfoTransportRefusal::Semantic(error))
    })?;
    let payload = encode_structured_transport(value, maximum_payload_bytes)
        .map_err(StructuredWireRefusal::Structured)?;
    Ok(ConnectionEnvelope {
        protocol_version: PROTOCOL_VERSION,
        plan_id,
        connection_id,
        sequence,
        value_kind: profile.value_kind().clone(),
        payload,
    })
}

pub fn structured_value_from_envelope(
    expected_type: &StructuredInfoType,
    envelope: &ConnectionEnvelope,
    maximum_payload_bytes: u32,
) -> Result<StructuredInfoValue, StructuredWireRefusal> {
    if envelope.protocol_version != PROTOCOL_VERSION {
        return Err(StructuredWireRefusal::Wire(WireError::WrongProtocolVersion));
    }
    let expected_profile = expected_type.profile().map_err(|error| {
        StructuredWireRefusal::Structured(StructuredInfoTransportRefusal::Semantic(error))
    })?;
    if envelope.value_kind != *expected_profile.value_kind() {
        return Err(StructuredWireRefusal::ProfileMismatch);
    }
    decode_structured_transport(expected_type, &envelope.payload, maximum_payload_bytes)
        .map_err(StructuredWireRefusal::Structured)
}
