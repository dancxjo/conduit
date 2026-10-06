//! Independent expectation for 128 alternating key reports and ordered batches.
use super::*;

pub(super) fn expected_digest(outputs: &[PortDescriptor]) -> Result<String, ConduitosError> {
    let source = PreparedProtocolSource::prepare(
        conduitos::protocol_source::usb_hid_keyboard_order_package()
            .map_err(|error| refusal("hid-capture-proof-source", format!("{error:?}")))?,
    )
    .map_err(|error| refusal("hid-capture-proof-source", format!("{error:?}")))?;
    let schema = |name: &str| {
        source
            .checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .map(|ty| ty.value_type.clone())
            .ok_or_else(|| refusal("hid-capture-proof-type", name))
    };
    let observation = schema("UsbKeyboardOrderedObservation")?;
    let report = schema("UsbBootKeyboardReport")?;
    let observed = schema("UsbBootKeyboardResult")?;
    let transition = schema("UsbKeyboardTransition")?;
    let batch = schema("UsbKeyboardTransitionBatch")?;
    let end = schema("UsbKeyboardOrderEnd")?;
    let keys_type = field_type(&report, "keys")?;
    let StructuredInfoTypeShape::Collection { element: key, .. } = keys_type.shape() else {
        return Err(refusal("hid-capture-proof-type", "keys collection"));
    };
    let slots_type = field_type(&batch, "slots")?;
    let StructuredInfoTypeShape::Collection { element: slot, .. } = slots_type.shape() else {
        return Err(refusal("hid-capture-proof-type", "slots collection"));
    };
    let mut digest = Sha256::new();
    for sequence in 0..128_u64 {
        let pressed = sequence.is_multiple_of(2);
        let keys = (0..6)
            .map(|index| {
                StructuredInfoValue::leaf(
                    key.clone(),
                    vec![if pressed && index == 0 { 4 } else { 0 }],
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(value_error)?;
        let keyboard = StructuredInfoValue::variant(
            observed.clone(),
            "keyboard",
            record(
                &report,
                vec![
                    leaf(&report, "modifiers", vec![0])?,
                    StructuredFieldValue::new(
                        "keys",
                        StructuredInfoValue::collection(keys_type.clone(), keys)
                            .map_err(value_error)?,
                    )
                    .map_err(value_error)?,
                ],
            )?,
        )
        .map_err(value_error)?;
        let observation_value = record(
            &observation,
            vec![
                leaf(&observation, "ordinal", sequence.to_le_bytes().to_vec())?,
                StructuredFieldValue::new("observed", keyboard).map_err(value_error)?,
            ],
        )?;
        let slots = (0..20)
            .map(|index| {
                if index == if pressed { 14 } else { 8 } {
                    StructuredInfoValue::variant(
                        slot.clone(),
                        "changed",
                        record(
                            &transition,
                            vec![
                                leaf(&transition, "usage", vec![4])?,
                                leaf(&transition, "pressed", vec![u8::from(pressed)])?,
                                leaf(&transition, "modifiers", vec![0])?,
                            ],
                        )?,
                    )
                    .map_err(value_error)
                } else {
                    unit_variant(slot, "unchanged")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let batch_value = record(
            &batch,
            vec![
                StructuredFieldValue::new(
                    "slots",
                    StructuredInfoValue::collection(slots_type.clone(), slots)
                        .map_err(value_error)?,
                )
                .map_err(value_error)?,
            ],
        )?;
        for port in outputs
            .iter()
            .filter(|port| port.port_id.as_str() != "ended")
        {
            let value = match port.port_id.as_str() {
                "observation" => &observation_value,
                "changes" => &batch_value,
                _ => return Err(refusal("hid-capture-proof-port", port.port_id.as_str())),
            };
            digest.update(sequence.to_le_bytes());
            digest.update(port.port_id.as_str().as_bytes());
            digest.update(value.canonical_bytes().map_err(value_error)?);
        }
    }
    let ended = outputs
        .iter()
        .find(|port| port.port_id.as_str() == "ended")
        .ok_or_else(|| refusal("hid-capture-proof-port", "ended"))?;
    digest.update(0_u64.to_le_bytes());
    digest.update(ended.port_id.as_str().as_bytes());
    digest.update(
        unit_variant(&end, "closed")?
            .canonical_bytes()
            .map_err(value_error)?,
    );
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn unit_variant(ty: &StructuredInfoType, tag: &str) -> Result<StructuredInfoValue, ConduitosError> {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        return Err(refusal("hid-capture-proof-type", "variant"));
    };
    let case = cases
        .iter()
        .find(|case| case.tag() == tag)
        .ok_or_else(|| refusal("hid-capture-proof-case", tag))?;
    StructuredInfoValue::variant(
        ty.clone(),
        tag,
        StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).map_err(value_error)?,
    )
    .map_err(value_error)
}
