use super::{
    schema::{request_from_value, request_value, response_from_value, response_value},
    HttpContractError, HttpRequest, HttpResponse, HTTP_MAXIMUM_ENCODED_REQUEST_BYTES,
    HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
};
use alloc::vec::Vec;
use conduit_core::StructuredInfoValue;

pub fn encode_request(value: &HttpRequest) -> Result<Vec<u8>, HttpContractError> {
    let encoded = request_value(value)?
        .canonical_bytes()
        .map_err(|_| HttpContractError::EncodedValueOverflow)?;
    bounded(encoded, HTTP_MAXIMUM_ENCODED_REQUEST_BYTES)
}

pub fn decode_request(encoded: &[u8]) -> Result<HttpRequest, HttpContractError> {
    check_bound(encoded, HTTP_MAXIMUM_ENCODED_REQUEST_BYTES)?;
    let structured = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| HttpContractError::MalformedEncoding)?;
    let value = request_from_value(&structured)?;
    value.validate()?;
    Ok(value)
}

pub fn encode_response(value: &HttpResponse) -> Result<Vec<u8>, HttpContractError> {
    let encoded = response_value(value)?
        .canonical_bytes()
        .map_err(|_| HttpContractError::EncodedValueOverflow)?;
    bounded(encoded, HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES)
}

pub fn decode_response(encoded: &[u8]) -> Result<HttpResponse, HttpContractError> {
    check_bound(encoded, HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES)?;
    let structured = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| HttpContractError::MalformedEncoding)?;
    let value = response_from_value(&structured)?;
    value.validate()?;
    Ok(value)
}

fn bounded(encoded: Vec<u8>, maximum: u32) -> Result<Vec<u8>, HttpContractError> {
    check_bound(&encoded, maximum)?;
    Ok(encoded)
}

fn check_bound(encoded: &[u8], maximum: u32) -> Result<(), HttpContractError> {
    if encoded.len() > maximum as usize {
        Err(HttpContractError::EncodedValueOverflow)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        http_request_type, HttpBody, HttpHeader, HttpMethod, HttpTarget, HttpTransactionId,
    };
    use alloc::vec;
    use conduit_core::{
        kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
        ResourceSemanticIdentity, ResourceVersionIdentity, StructuredSelection, StructuredSelector,
        UnmatchedVariantDisposition,
    };

    fn request() -> HttpRequest {
        HttpRequest {
            transaction_id: HttpTransactionId(42),
            method: HttpMethod::Post,
            target: HttpTarget {
                scheme: "https".into(),
                authority: "api.example.test".into(),
                path_and_query: "/v1/items?q=one".into(),
            },
            headers: vec![
                HttpHeader {
                    name: "x-order".into(),
                    value: b"first".to_vec(),
                },
                HttpHeader {
                    name: "x-order".into(),
                    value: b"second".to_vec(),
                },
            ],
            body: HttpBody::inline(b"bounded".to_vec()),
        }
    }

    #[test]
    fn request_round_trip_is_canonical_structured_info() {
        let value = request();
        let encoded = encode_request(&value).unwrap();
        let structured = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
        assert_eq!(structured.value_type(), &crate::http_request_type());
        assert_eq!(decode_request(&encoded).unwrap(), value);
    }

    #[test]
    fn header_name_is_selected_without_protocol_or_text_parsing() {
        let structured =
            StructuredInfoValue::from_canonical_bytes(&encode_request(&request()).unwrap())
                .unwrap();
        let headers = matched(
            StructuredSelector::field(http_request_type(), "headers")
                .unwrap()
                .select(&structured)
                .unwrap(),
        );
        let first = matched(
            StructuredSelector::index(headers.value_type().clone(), 0)
                .unwrap()
                .select(&headers)
                .unwrap(),
        );
        let header = matched(
            StructuredSelector::variant(
                first.value_type().clone(),
                "header",
                UnmatchedVariantDisposition::Refuse,
            )
            .unwrap()
            .select(&first)
            .unwrap(),
        );
        let name = matched(
            StructuredSelector::field(header.value_type().clone(), "name")
                .unwrap()
                .select(&header)
                .unwrap(),
        );
        assert!(matches!(
            name.shape(),
            conduit_core::StructuredInfoValueShape::Leaf(b"x-order")
        ));
    }

    fn matched(selection: StructuredSelection) -> StructuredInfoValue {
        match selection {
            StructuredSelection::Matched(value) => value,
            StructuredSelection::Unmatched(_) => panic!("expected exact HTTP selection"),
        }
    }

    #[test]
    fn response_status_is_data_including_500() {
        let value = HttpResponse {
            transaction_id: HttpTransactionId(42),
            status: 500,
            headers: Vec::new(),
            body: HttpBody::inline(b"error document".to_vec()),
        };
        assert_eq!(
            decode_response(&encode_response(&value).unwrap()).unwrap(),
            value
        );
    }

    #[test]
    fn bounded_resource_body_round_trips_without_a_path_or_url() {
        let reference = BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: kind_id("media/http-body@1"),
            access_class: ResourceClassId::from("resource/http-body"),
            extent: ResourceExtent {
                bytes: 8_000_000,
                items: None,
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        };
        let mut value = request();
        value.body = HttpBody::Resource(reference);
        assert_eq!(
            decode_request(&encode_request(&value).unwrap()).unwrap(),
            value
        );
    }

    #[test]
    fn wrong_profile_and_trailing_bytes_refuse() {
        assert_eq!(
            decode_request(&[1]),
            Err(HttpContractError::MalformedEncoding)
        );
        let mut encoded = encode_request(&request()).unwrap();
        encoded.push(0);
        assert_eq!(
            decode_request(&encoded),
            Err(HttpContractError::MalformedEncoding)
        );
    }

    #[test]
    fn invalid_headers_and_inline_body_overflow_refuse() {
        let mut value = request();
        value.headers[0].name = "Upper".into();
        assert_eq!(value.validate(), Err(HttpContractError::InvalidHeaderName));
        value = request();
        value.body = HttpBody::Inline(vec![0; crate::HTTP_MAXIMUM_REQUEST_BODY_BYTES + 1]);
        assert_eq!(
            value.validate(),
            Err(HttpContractError::RequestBodyOverflow)
        );
    }

    #[test]
    fn credentials_framing_and_cookies_stay_out_of_ordinary_headers() {
        for name in [
            "authorization",
            "proxy-authorization",
            "cookie",
            "set-cookie",
        ] {
            let mut value = request();
            value.headers[0].name = name.into();
            assert_eq!(
                value.validate(),
                Err(HttpContractError::SensitiveHeaderRequiresProtectedPath)
            );
        }
        for name in ["content-length", "transfer-encoding"] {
            let mut value = request();
            value.headers[0].name = name.into();
            assert_eq!(
                value.validate(),
                Err(HttpContractError::FramingHeaderIsDerived)
            );
        }
    }
}
