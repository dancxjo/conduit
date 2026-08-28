//! Exact bounded structured Info carried by an ordinary connection envelope.

use conduit_core::{
    decode_self_describing_structured_transport, decode_structured_transport,
    encode_structured_transport, ConnectionEnvelope, ConnectionId, PlanId,
    StructuredInfoTransportRefusal, StructuredInfoType, StructuredInfoValue, PROTOCOL_VERSION,
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

/// Convert runtime-local canonical structured bytes into the exact Line form.
pub fn structured_transport_envelope_from_local(
    mut envelope: ConnectionEnvelope,
    maximum_payload_bytes: u32,
) -> Result<ConnectionEnvelope, StructuredWireRefusal> {
    let value = StructuredInfoValue::from_canonical_bytes(&envelope.payload).map_err(|error| {
        StructuredWireRefusal::Structured(StructuredInfoTransportRefusal::Semantic(error))
    })?;
    let profile = value.value_type().profile().map_err(|error| {
        StructuredWireRefusal::Structured(StructuredInfoTransportRefusal::Semantic(error))
    })?;
    if envelope.value_kind != *profile.value_kind() {
        return Err(StructuredWireRefusal::ProfileMismatch);
    }
    envelope.payload = encode_structured_transport(&value, maximum_payload_bytes)
        .map_err(StructuredWireRefusal::Structured)?;
    Ok(envelope)
}

/// Validate a Line representation and restore runtime-local canonical bytes.
pub fn structured_local_envelope_from_transport(
    mut envelope: ConnectionEnvelope,
    maximum_payload_bytes: u32,
) -> Result<ConnectionEnvelope, StructuredWireRefusal> {
    let value =
        decode_self_describing_structured_transport(&envelope.payload, maximum_payload_bytes)
            .map_err(StructuredWireRefusal::Structured)?;
    let profile = value.value_type().profile().map_err(|error| {
        StructuredWireRefusal::Structured(StructuredInfoTransportRefusal::Semantic(error))
    })?;
    if envelope.value_kind != *profile.value_kind() {
        return Err(StructuredWireRefusal::ProfileMismatch);
    }
    envelope.payload = value.canonical_bytes().map_err(|error| {
        StructuredWireRefusal::Structured(StructuredInfoTransportRefusal::Semantic(error))
    })?;
    Ok(envelope)
}
