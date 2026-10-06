//! Independent expectation for the finite QMP mouse input fixture.
//! QEMU 10.2.1 hw/input/hid.c preserves X/Y and emits a fourth wheel octet:
//! https://github.com/qemu/qemu/blob/v10.2.1/hw/input/hid.c
use super::*;

pub(super) fn expected_digest(
    source: &PreparedProtocolSource,
    outputs: &[PortDescriptor],
) -> Result<String, ConduitosError> {
    let schema = |name: &str| {
        source
            .checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .map(|ty| ty.value_type.clone())
            .ok_or_else(|| refusal("hid-proof-type", name))
    };
    if outputs.len() != 1 || outputs[0].port_id.as_str() != "event" {
        return Err(refusal("hid-proof-port", "mouse event"));
    }
    let sample = schema("UsbMousePointerSample")?;
    let sample_event = schema("UsbMousePointerSampleEvent")?;
    let event = schema("UsbMousePointerEvent")?;
    let end = schema("UsbMouseOrderEnd")?;
    let mut digest = Sha256::new();
    let mut position_x = 500000_i64;
    let mut position_y = 500000_i64;
    for ordinal in 0..128_u64 {
        let pressed = ordinal.is_multiple_of(2);
        let dx: i64 = if pressed { 4000 } else { -4000 };
        let dy: i64 = if pressed { 8000 } else { -8000 };
        position_x += dx;
        position_y += dy;
        let normalized = record(
            &sample,
            vec![
                leaf(&sample, "position-x", position_x.to_le_bytes().to_vec())?,
                leaf(&sample, "position-y", position_y.to_le_bytes().to_vec())?,
                leaf(&sample, "delta-x", dx.to_le_bytes().to_vec())?,
                leaf(&sample, "delta-y", dy.to_le_bytes().to_vec())?,
                leaf(&sample, "primary-pressed", vec![u8::from(pressed)])?,
                leaf(&sample, "sequence", (ordinal + 1).to_le_bytes().to_vec())?,
                leaf(&sample, "queue-capacity", 8_u64.to_le_bytes().to_vec())?,
                leaf(&sample, "coalesced", 0_u64.to_le_bytes().to_vec())?,
                leaf(&sample, "dropped", 0_u64.to_le_bytes().to_vec())?,
            ],
        )?;
        let value = StructuredInfoValue::variant(
            event.clone(),
            "sample",
            record(
                &sample_event,
                vec![
                    leaf(&sample_event, "ordinal", ordinal.to_le_bytes().to_vec())?,
                    StructuredFieldValue::new("sample", normalized).map_err(value_error)?,
                ],
            )?,
        )
        .map_err(value_error)?;
        digest.update(ordinal.to_le_bytes());
        digest.update(b"event");
        digest.update(value.canonical_bytes().map_err(value_error)?);
    }
    let unit = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/unit")).map_err(value_error)?,
        vec![],
    )
    .map_err(value_error)?;
    let closed = StructuredInfoValue::variant(end, "closed", unit).map_err(value_error)?;
    let value = StructuredInfoValue::variant(event, "ended", closed).map_err(value_error)?;
    digest.update(128_u64.to_le_bytes());
    digest.update(b"event");
    digest.update(value.canonical_bytes().map_err(value_error)?);
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
