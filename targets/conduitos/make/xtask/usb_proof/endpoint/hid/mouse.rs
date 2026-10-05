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
    let frame = schema("UsbHidReportFrame")?;
    let transfer = schema("UsbHidTransferFrame")?;
    let report = schema("UsbBootMouseReport")?;
    let observed = schema("UsbBootMouseResult")?;
    let mut digest = Sha256::new();
    for sequence in 0..128_u64 {
        let pressed = sequence % 2 == 0;
        let x: i16 = if pressed { 1 } else { -1 };
        let y: i16 = if pressed { 2 } else { -2 };
        let wire = vec![u8::from(pressed), x as u8, y as u8, 0];
        let received = StructuredInfoValue::variant(
            transfer.clone(),
            "frame",
            record(
                &frame,
                vec![
                    leaf(&frame, "wire", wire.clone())?,
                    leaf(&frame, "actual", 4_u64.to_le_bytes().to_vec())?,
                ],
            )?,
        )
        .map_err(value_error)?;
        let mouse = StructuredInfoValue::variant(
            observed.clone(),
            "mouse",
            record(
                &report,
                vec![
                    leaf(&report, "buttons", vec![u8::from(pressed)])?,
                    leaf(&report, "x", x.to_le_bytes().to_vec())?,
                    leaf(&report, "y", y.to_le_bytes().to_vec())?,
                    leaf(&report, "wire", wire)?,
                ],
            )?,
        )
        .map_err(value_error)?;
        for port in outputs {
            let value = match port.port_id.as_str() {
                "received" => &received,
                "decoded" => &mouse,
                _ => return Err(refusal("hid-proof-port", port.port_id.as_str())),
            };
            digest.update(sequence.to_le_bytes());
            digest.update(port.port_id.as_str().as_bytes());
            digest.update(value.canonical_bytes().map_err(value_error)?);
        }
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
