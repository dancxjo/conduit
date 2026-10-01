use super::{
    HttpContractError, HttpRequest, HttpResponse, HTTP_MAXIMUM_ENCODED_REQUEST_BYTES,
    HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
};
use alloc::vec::Vec;
use conduit_core::StructuredInfoValue;
use conduit_form::rust_binding::NativeRustBinding;

pub fn encode_request(value: &HttpRequest) -> Result<Vec<u8>, HttpContractError> {
    value.validate()?;
    let encoded = value
        .clone()
        .into_structured()
        .map_err(|_| HttpContractError::MalformedEncoding)?
        .canonical_bytes()
        .map_err(|_| HttpContractError::EncodedValueOverflow)?;
    bounded(encoded, HTTP_MAXIMUM_ENCODED_REQUEST_BYTES)
}

pub fn decode_request(encoded: &[u8]) -> Result<HttpRequest, HttpContractError> {
    check_bound(encoded, HTTP_MAXIMUM_ENCODED_REQUEST_BYTES)?;
    let structured = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| HttpContractError::MalformedEncoding)?;
    let value = HttpRequest::from_structured(structured)
        .map_err(|_| HttpContractError::MalformedEncoding)?;
    value.validate()?;
    Ok(value)
}

pub fn encode_response(value: &HttpResponse) -> Result<Vec<u8>, HttpContractError> {
    value.validate()?;
    let encoded = value
        .clone()
        .into_structured()
        .map_err(|_| HttpContractError::MalformedEncoding)?
        .canonical_bytes()
        .map_err(|_| HttpContractError::EncodedValueOverflow)?;
    bounded(encoded, HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES)
}

pub fn decode_response(encoded: &[u8]) -> Result<HttpResponse, HttpContractError> {
    check_bound(encoded, HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES)?;
    let structured = StructuredInfoValue::from_canonical_bytes(encoded)
        .map_err(|_| HttpContractError::MalformedEncoding)?;
    let value = HttpResponse::from_structured(structured)
        .map_err(|_| HttpContractError::MalformedEncoding)?;
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
    };

    fn transaction_id(value: u64) -> HttpTransactionId {
        HttpTransactionId::new(value).unwrap()
    }

    fn header(name: &str, value: &[u8]) -> HttpHeader {
        HttpHeader::new(
            name.into(),
            conduit_form::rust_binding::BoundedBytes::new(value).unwrap(),
        )
        .unwrap()
    }

    fn request() -> HttpRequest {
        HttpRequest {
            transaction_id: transaction_id(42),
            method: HttpMethod::Post,
            target: HttpTarget::new(
                "api.example.test".into(),
                "/v1/items?q=one".into(),
                crate::HttpScheme::Https,
            )
            .unwrap(),
            headers: crate::http_headers([
                header("x-order", b"first"),
                header("x-order", b"second"),
            ])
            .unwrap(),
            body: HttpBody::inline(b"bounded"),
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
        let conduit_core::StructuredInfoValueShape::Collection(items) = headers.shape() else {
            panic!("HTTP headers remain a finite structured sequence")
        };
        let first = items[0].clone();
        let name = matched(
            StructuredSelector::field(first.value_type().clone(), "name")
                .unwrap()
                .select(&first)
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
            transaction_id: transaction_id(42),
            status: 500,
            headers: Default::default(),
            body: HttpBody::inline(b"error document"),
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
        value.headers =
            crate::http_headers([header("Upper", b"first"), header("x-order", b"second")]).unwrap();
        assert_eq!(value.validate(), Err(HttpContractError::InvalidHeaderName));
        assert!(conduit_form::rust_binding::BoundedBytes::<
            { crate::HTTP_MAXIMUM_REQUEST_BODY_BYTES },
        >::new(&vec![0; crate::HTTP_MAXIMUM_REQUEST_BODY_BYTES + 1])
        .is_none());
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
            value.headers =
                crate::http_headers([header(name, b"first"), header("x-order", b"second")])
                    .unwrap();
            assert_eq!(
                value.validate(),
                Err(HttpContractError::SensitiveHeaderRequiresProtectedPath)
            );
        }
        for name in ["content-length", "transfer-encoding"] {
            let mut value = request();
            value.headers =
                crate::http_headers([header(name, b"first"), header("x-order", b"second")])
                    .unwrap();
            assert_eq!(
                value.validate(),
                Err(HttpContractError::FramingHeaderIsDerived)
            );
        }
    }
}
