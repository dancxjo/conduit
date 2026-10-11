//! Canonical frozen discovery/selection information, independent of class policy.
use super::super::usb_request::{UsbEndpointBootRequest, UsbProtocolBootRequest};
use crate::{arch::UsbConfigurationSnapshot, usb_base::control_contract::ControlContract};
use alloc::vec::Vec;
use conduit_core::*;
use sha2::{Digest, Sha256};
const MAXIMUM_CARRIER_BYTES: usize = 4096;

pub(super) fn request_bytes(request: &UsbProtocolBootRequest) -> Result<Vec<u8>, &'static str> {
    let bytes = serde_json::to_vec(request).map_err(|_| "protocol-usb-preflight-root-encoding")?;
    if bytes.len() > super::super::MAXIMUM_REQUEST_BYTES {
        return Err("protocol-usb-preflight-root-bound");
    }
    Ok(bytes)
}
pub(super) fn request_digest(request: &UsbProtocolBootRequest) -> Result<[u8; 32], &'static str> {
    Ok(Sha256::digest(request_bytes(request)?).into())
}
fn fields<'a>(
    schema: &'a StructuredInfoType,
    names: &[&str],
) -> Result<Vec<&'a StructuredInfoType>, &'static str> {
    let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
        return Err("protocol-usb-preflight-carrier-type");
    };
    if fields.len() != names.len()
        || fields
            .iter()
            .zip(names)
            .any(|(field, name)| field.name() != *name)
    {
        return Err("protocol-usb-preflight-carrier-type");
    }
    Ok(fields.iter().map(|field| field.value_type()).collect())
}
fn endpoint(
    schema: &StructuredInfoType,
    selected: Option<&UsbEndpointBootRequest>,
) -> Result<Vec<u8>, &'static str> {
    let refuse = |_| "protocol-usb-preflight-carrier-type";
    let StructuredInfoTypeShape::Variant { cases, .. } = schema.shape() else {
        return Err("protocol-usb-preflight-carrier-type");
    };
    if cases.len() != 2 || cases[0].tag() != "absent" || cases[1].tag() != "selected" {
        return Err("protocol-usb-preflight-carrier-type");
    }
    let u64_type = StructuredInfoType::leaf(kind_id("value/u64")).map_err(refuse)?;
    let empty_type = StructuredInfoType::leaf(kind_id("value/empty")).map_err(refuse)?;
    if cases[0].payload_type() != &empty_type {
        return Err("protocol-usb-preflight-carrier-type");
    }
    let geometry = cases[1].payload_type();
    if fields(
        geometry,
        &["address", "epoch", "interval", "packet", "transfer"],
    )?
    .iter()
    .any(|field| **field != u64_type)
    {
        return Err("protocol-usb-preflight-carrier-type");
    }
    let mut variant =
        PreparedStructuredComposer::new(schema, MAXIMUM_CARRIER_BYTES).map_err(refuse)?;
    let Some(selected) = selected else {
        let mut empty = PreparedStructuredComposer::new(&empty_type, 64).map_err(refuse)?;
        return Ok(variant
            .variant(
                "absent",
                validate_canonical_structured_value(empty.leaf(&[]).map_err(refuse)?)
                    .map_err(refuse)?,
            )
            .map_err(refuse)?
            .to_vec());
    };
    let mut primitive = PreparedStructuredComposer::new(&u64_type, 64).map_err(refuse)?;
    let values: Vec<Vec<u8>> = [
        u64::from(selected.address),
        u64::from(selected.endpoint_epoch),
        u64::from(selected.interval),
        u64::from(selected.maximum_packet),
        u64::from(selected.transfer_type),
    ]
    .iter()
    .map(|value| {
        primitive
            .leaf(&value.to_le_bytes())
            .map(|bytes| bytes.to_vec())
            .map_err(refuse)
    })
    .collect::<Result<_, _>>()?;
    let children = values
        .iter()
        .map(|bytes| validate_canonical_structured_value(bytes).map_err(refuse))
        .collect::<Result<Vec<_>, _>>()?;
    let mut record =
        PreparedStructuredComposer::new(geometry, MAXIMUM_CARRIER_BYTES).map_err(refuse)?;
    let body = validate_canonical_structured_value(record.record(&children).map_err(refuse)?)
        .map_err(refuse)?;
    Ok(variant.variant("selected", body).map_err(refuse)?.to_vec())
}
pub(super) fn encode(
    schema: &StructuredInfoType,
    snapshot: &UsbConfigurationSnapshot,
    request: &UsbProtocolBootRequest,
    contract: &ControlContract,
) -> Result<Vec<u8>, &'static str> {
    let refuse = |_| "protocol-usb-preflight-carrier-type";
    let members = fields(schema, &["attachment", "configuration", "read", "write"])?;
    let u64_type = StructuredInfoType::leaf(kind_id("value/u64")).map_err(refuse)?;
    if members[0] != &u64_type || members[1] != contract.result_type() || members[2] != members[3] {
        return Err("protocol-usb-preflight-carrier-type");
    }
    let length = snapshot.requested();
    let raw = crate::usb_base::control_request::ControlTransferRequest::new(
        [0x80, 6, 0, 2, 0, 0, length as u8, (length >> 8) as u8],
        &[],
        256,
    )
    .map_err(|_| "protocol-usb-preflight-observation")?;
    let mut encoder = crate::usb_base::control_result::PreparedControlResultEncoder::new(contract)
        .map_err(refuse)?;
    let configuration = encoder
        .completed(&raw, snapshot.bytes().len() as u16, snapshot.bytes())
        .map_err(|_| "protocol-usb-preflight-observation")?;
    let mut primitive = PreparedStructuredComposer::new(&u64_type, 64).map_err(refuse)?;
    let attachment = primitive
        .leaf(&snapshot.facts()[10].to_le_bytes())
        .map_err(refuse)?;
    let read = endpoint(members[2], request.endpoint_read.as_ref())?;
    let write = endpoint(members[3], request.endpoint_write.as_ref())?;
    let mut record =
        PreparedStructuredComposer::new(schema, MAXIMUM_CARRIER_BYTES).map_err(refuse)?;
    Ok(record
        .record(&[
            validate_canonical_structured_value(attachment).map_err(refuse)?,
            validate_canonical_structured_value(configuration).map_err(refuse)?,
            validate_canonical_structured_value(&read).map_err(refuse)?,
            validate_canonical_structured_value(&write).map_err(refuse)?,
        ])
        .map_err(refuse)?
        .to_vec())
}
