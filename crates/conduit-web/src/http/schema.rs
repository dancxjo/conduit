//! Exact structured Info schemas for HTTP exchange data.

use super::{
    HttpBody, HttpContractError, HttpHeader, HttpMethod, HttpRequest, HttpResponse, HttpTarget,
    HttpTransactionId, HTTP_MAXIMUM_HEADERS,
};
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, BoundedResourceRef, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoValue, StructuredInfoValueShape, StructuredVariantCase,
    RESOURCE_REFERENCE_INFO_ID,
};

const COUNT: &str = "value/count@1";
const TEXT: &str = "value/text@1";
const BYTES: &str = "value/bytes@1";
const UNIT: &str = "value/unit@1";

fn leaf(id: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(id)).expect("reviewed HTTP leaf identity is finite")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed HTTP field is finite")
}

fn case(tag: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(tag, payload_type).expect("reviewed HTTP case is finite")
}

fn value_field(name: &str, value: StructuredInfoValue) -> StructuredFieldValue {
    StructuredFieldValue::new(name, value).expect("reviewed HTTP field value is finite")
}

fn target_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("http/target@1"),
        vec![
            field("authority", leaf(TEXT)),
            field("path_and_query", leaf(TEXT)),
            field("scheme", leaf(TEXT)),
        ],
    )
    .expect("HTTP target schema is finite")
}

fn header_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("http/header@1"),
        vec![field("name", leaf(TEXT)), field("value", leaf(BYTES))],
    )
    .expect("HTTP header schema is finite")
}

fn header_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("http/header-slot@1"),
        vec![case("header", header_type()), case("unused", leaf(UNIT))],
    )
    .expect("HTTP header slot schema is finite")
}

fn headers_type() -> StructuredInfoType {
    StructuredInfoType::collection(header_slot_type(), Some(HTTP_MAXIMUM_HEADERS as u16))
        .expect("HTTP header table has an exact finite length")
}

fn method_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("http/method@1"),
        ["delete", "get", "head", "options", "patch", "post", "put"]
            .into_iter()
            .map(|tag| case(tag, leaf(UNIT)))
            .collect(),
    )
    .expect("HTTP method schema is finite")
}

fn body_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("http/body@1"),
        vec![
            case("inline", leaf(BYTES)),
            case("resource", leaf(RESOURCE_REFERENCE_INFO_ID)),
        ],
    )
    .expect("HTTP body schema is finite")
}

pub fn http_request_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("http/request@2"),
        vec![
            field("body", body_type()),
            field("headers", headers_type()),
            field("method", method_type()),
            field("target", target_type()),
            field("transaction_id", leaf(COUNT)),
        ],
    )
    .expect("HTTP request schema is finite")
}

pub fn http_response_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("http/response@2"),
        vec![
            field("body", body_type()),
            field("headers", headers_type()),
            field("status", leaf(COUNT)),
            field("transaction_id", leaf(COUNT)),
        ],
    )
    .expect("HTTP response schema is finite")
}

pub(super) fn request_value(
    request: &HttpRequest,
) -> Result<StructuredInfoValue, HttpContractError> {
    request.validate()?;
    record(
        http_request_type(),
        vec![
            value_field("body", body_value(&request.body)?),
            value_field("headers", headers_value(&request.headers)?),
            value_field("method", method_value(request.method)?),
            value_field("target", target_value(&request.target)?),
            value_field("transaction_id", count_value(request.transaction_id.0)?),
        ],
    )
}

pub(super) fn response_value(
    response: &HttpResponse,
) -> Result<StructuredInfoValue, HttpContractError> {
    response.validate()?;
    record(
        http_response_type(),
        vec![
            value_field("body", body_value(&response.body)?),
            value_field("headers", headers_value(&response.headers)?),
            value_field("status", count_value(u64::from(response.status))?),
            value_field("transaction_id", count_value(response.transaction_id.0)?),
        ],
    )
}

fn record(
    value_type: StructuredInfoType,
    fields: Vec<StructuredFieldValue>,
) -> Result<StructuredInfoValue, HttpContractError> {
    StructuredInfoValue::record(value_type, fields)
        .map_err(|_| HttpContractError::EncodedValueOverflow)
}

fn leaf_value(
    value_type: StructuredInfoType,
    bytes: Vec<u8>,
) -> Result<StructuredInfoValue, HttpContractError> {
    StructuredInfoValue::leaf(value_type, bytes)
        .map_err(|_| HttpContractError::EncodedValueOverflow)
}

fn count_value(value: u64) -> Result<StructuredInfoValue, HttpContractError> {
    leaf_value(leaf(COUNT), value.to_le_bytes().to_vec())
}

fn text_value(value: &str) -> Result<StructuredInfoValue, HttpContractError> {
    leaf_value(leaf(TEXT), value.as_bytes().to_vec())
}

fn target_value(target: &HttpTarget) -> Result<StructuredInfoValue, HttpContractError> {
    record(
        target_type(),
        vec![
            value_field("authority", text_value(&target.authority)?),
            value_field("path_and_query", text_value(&target.path_and_query)?),
            value_field("scheme", text_value(&target.scheme)?),
        ],
    )
}

fn method_value(method: HttpMethod) -> Result<StructuredInfoValue, HttpContractError> {
    let tag = match method {
        HttpMethod::Get => "get",
        HttpMethod::Head => "head",
        HttpMethod::Post => "post",
        HttpMethod::Put => "put",
        HttpMethod::Patch => "patch",
        HttpMethod::Delete => "delete",
        HttpMethod::Options => "options",
    };
    let unit = leaf_value(leaf(UNIT), Vec::new())?;
    StructuredInfoValue::variant(method_type(), tag, unit)
        .map_err(|_| HttpContractError::MalformedEncoding)
}

fn headers_value(headers: &[HttpHeader]) -> Result<StructuredInfoValue, HttpContractError> {
    let mut slots = Vec::with_capacity(HTTP_MAXIMUM_HEADERS);
    for header in headers {
        let value = record(
            header_type(),
            vec![
                value_field("name", text_value(&header.name)?),
                value_field("value", leaf_value(leaf(BYTES), header.value.clone())?),
            ],
        )?;
        slots.push(
            StructuredInfoValue::variant(header_slot_type(), "header", value)
                .map_err(|_| HttpContractError::MalformedEncoding)?,
        );
    }
    while slots.len() < HTTP_MAXIMUM_HEADERS {
        let unit = leaf_value(leaf(UNIT), Vec::new())?;
        slots.push(
            StructuredInfoValue::variant(header_slot_type(), "unused", unit)
                .map_err(|_| HttpContractError::MalformedEncoding)?,
        );
    }
    StructuredInfoValue::collection(headers_type(), slots)
        .map_err(|_| HttpContractError::MalformedEncoding)
}

fn body_value(body: &HttpBody) -> Result<StructuredInfoValue, HttpContractError> {
    let (tag, payload) = match body {
        HttpBody::Inline(bytes) => ("inline", leaf_value(leaf(BYTES), bytes.clone())?),
        HttpBody::Resource(reference) => (
            "resource",
            leaf_value(
                leaf(RESOURCE_REFERENCE_INFO_ID),
                reference
                    .encode()
                    .map_err(|_| HttpContractError::MalformedEncoding)?,
            )?,
        ),
    };
    StructuredInfoValue::variant(body_type(), tag, payload)
        .map_err(|_| HttpContractError::MalformedEncoding)
}

pub(super) fn request_from_value(
    value: &StructuredInfoValue,
) -> Result<HttpRequest, HttpContractError> {
    if value.value_type() != &http_request_type() {
        return Err(HttpContractError::MalformedEncoding);
    }
    let fields = record_fields(value)?;
    Ok(HttpRequest {
        transaction_id: HttpTransactionId(count_field(fields, "transaction_id")?),
        method: decode_method(field_value(fields, "method")?)?,
        target: decode_target(field_value(fields, "target")?)?,
        headers: decode_headers(field_value(fields, "headers")?)?,
        body: decode_body(field_value(fields, "body")?)?,
    })
}

pub(super) fn response_from_value(
    value: &StructuredInfoValue,
) -> Result<HttpResponse, HttpContractError> {
    if value.value_type() != &http_response_type() {
        return Err(HttpContractError::MalformedEncoding);
    }
    let fields = record_fields(value)?;
    Ok(HttpResponse {
        transaction_id: HttpTransactionId(count_field(fields, "transaction_id")?),
        status: u16::try_from(count_field(fields, "status")?)
            .map_err(|_| HttpContractError::InvalidStatus)?,
        headers: decode_headers(field_value(fields, "headers")?)?,
        body: decode_body(field_value(fields, "body")?)?,
    })
}

fn record_fields(
    value: &StructuredInfoValue,
) -> Result<&[StructuredFieldValue], HttpContractError> {
    match value.shape() {
        StructuredInfoValueShape::Record(fields) => Ok(fields),
        _ => Err(HttpContractError::MalformedEncoding),
    }
}

fn field_value<'a>(
    fields: &'a [StructuredFieldValue],
    name: &str,
) -> Result<&'a StructuredInfoValue, HttpContractError> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(HttpContractError::MalformedEncoding)
}

fn leaf_bytes(value: &StructuredInfoValue) -> Result<&[u8], HttpContractError> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => Ok(bytes),
        _ => Err(HttpContractError::MalformedEncoding),
    }
}

fn count_field(fields: &[StructuredFieldValue], name: &str) -> Result<u64, HttpContractError> {
    leaf_bytes(field_value(fields, name)?)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| HttpContractError::MalformedEncoding)
}

fn decode_method(value: &StructuredInfoValue) -> Result<HttpMethod, HttpContractError> {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        return Err(HttpContractError::MalformedEncoding);
    };
    match tag {
        "get" => Ok(HttpMethod::Get),
        "head" => Ok(HttpMethod::Head),
        "post" => Ok(HttpMethod::Post),
        "put" => Ok(HttpMethod::Put),
        "patch" => Ok(HttpMethod::Patch),
        "delete" => Ok(HttpMethod::Delete),
        "options" => Ok(HttpMethod::Options),
        _ => Err(HttpContractError::MalformedEncoding),
    }
}

fn decode_target(value: &StructuredInfoValue) -> Result<HttpTarget, HttpContractError> {
    let fields = record_fields(value)?;
    Ok(HttpTarget {
        scheme: decode_text(field_value(fields, "scheme")?)?,
        authority: decode_text(field_value(fields, "authority")?)?,
        path_and_query: decode_text(field_value(fields, "path_and_query")?)?,
    })
}

fn decode_text(value: &StructuredInfoValue) -> Result<String, HttpContractError> {
    String::from_utf8(leaf_bytes(value)?.to_vec()).map_err(|_| HttpContractError::MalformedEncoding)
}

fn decode_headers(value: &StructuredInfoValue) -> Result<Vec<HttpHeader>, HttpContractError> {
    let StructuredInfoValueShape::Collection(slots) = value.shape() else {
        return Err(HttpContractError::MalformedEncoding);
    };
    let mut headers = Vec::new();
    let mut unused_seen = false;
    for slot in slots {
        let StructuredInfoValueShape::Variant { tag, payload } = slot.shape() else {
            return Err(HttpContractError::MalformedEncoding);
        };
        match tag {
            "unused" => unused_seen = true,
            "header" if !unused_seen => {
                let fields = record_fields(payload)?;
                headers.push(HttpHeader {
                    name: decode_text(field_value(fields, "name")?)?,
                    value: leaf_bytes(field_value(fields, "value")?)?.to_vec(),
                });
            }
            _ => return Err(HttpContractError::MalformedEncoding),
        }
    }
    Ok(headers)
}

fn decode_body(value: &StructuredInfoValue) -> Result<HttpBody, HttpContractError> {
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        return Err(HttpContractError::MalformedEncoding);
    };
    match tag {
        "inline" => Ok(HttpBody::Inline(leaf_bytes(payload)?.to_vec())),
        "resource" => Ok(HttpBody::Resource(
            BoundedResourceRef::decode(leaf_bytes(payload)?)
                .map_err(|_| HttpContractError::MalformedEncoding)?,
        )),
        _ => Err(HttpContractError::MalformedEncoding),
    }
}
