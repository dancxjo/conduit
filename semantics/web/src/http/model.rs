#[allow(dead_code, clippy::large_enum_variant)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    HttpBody, HttpContractError, HttpExchangeFailure, HttpHeader, HttpMethod, HttpRequest,
    HttpResponse, HttpScheme, HttpServerResponseRefusal, HttpTarget, HttpTransactionId,
    JsonCollectionRefusal, JsonRefusal, JsonSummaryRefusal,
};

pub const HTTP_MAXIMUM_IN_FLIGHT: u16 = 4;
pub const HTTP_MAXIMUM_HEADERS: usize = 16;
pub const HTTP_MAXIMUM_HEADER_NAME_BYTES: usize = 64;
pub const HTTP_MAXIMUM_HEADER_VALUE_BYTES: usize = 512;
pub const HTTP_MAXIMUM_AUTHORITY_BYTES: usize = 255;
pub const HTTP_MAXIMUM_TARGET_BYTES: usize = 2_048;
pub const HTTP_MAXIMUM_INLINE_BODY_BYTES: usize = 4_096;
pub const HTTP_MAXIMUM_REQUEST_BODY_BYTES: usize = HTTP_MAXIMUM_INLINE_BODY_BYTES;
pub const HTTP_MAXIMUM_RESPONSE_BODY_BYTES: usize = HTTP_MAXIMUM_INLINE_BODY_BYTES;
pub const HTTP_MAXIMUM_ENCODED_REQUEST_BYTES: u32 = 32_768;
pub const HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES: u32 = 32_768;
pub const HTTP_MAXIMUM_WORK_UNITS_PER_STEP: u16 = 1;
pub const HTTP_MANDATORY_SIGN_ITEMS_PER_TRANSACTION: u16 = 3;
pub const HTTP_AUTOMATIC_REDIRECTS: bool = false;
pub const HTTP_AUTOMATIC_RETRIES: bool = false;
pub const HTTP_AMBIENT_COOKIES: bool = false;
pub const HTTP_AMBIENT_CREDENTIALS: bool = false;
pub const HTTP_IMPLICIT_CACHING: bool = false;
pub const HTTP_IMPLICIT_DECOMPRESSION: bool = false;

pub type HttpHeaders =
    conduit_form::rust_binding::BoundedSequence<HttpHeader, HTTP_MAXIMUM_HEADERS>;

pub fn http_headers(
    values: impl IntoIterator<Item = HttpHeader>,
) -> Result<HttpHeaders, HttpContractError> {
    HttpHeaders::try_from_iter(values).map_err(|_| HttpContractError::TooManyHeaders)
}

pub fn http_request_type() -> conduit_core::StructuredInfoType {
    <HttpRequest as conduit_form::rust_binding::NativeRustBinding>::semantic_type()
        .expect("checked HTTP request Type is finite")
}

pub fn http_response_type() -> conduit_core::StructuredInfoType {
    <HttpResponse as conduit_form::rust_binding::NativeRustBinding>::semantic_type()
        .expect("checked HTTP response Type is finite")
}

impl HttpScheme {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

impl HttpBody {
    pub fn inline(bytes: impl AsRef<[u8]>) -> Self {
        Self::InlineBytes(
            conduit_form::rust_binding::BoundedBytes::new(bytes.as_ref())
                .expect("HTTP inline body must fit its semantic bound"),
        )
    }

    pub fn as_inline(&self) -> Option<&[u8]> {
        match self {
            Self::InlineBytes(bytes) => Some(bytes.as_slice()),
            Self::Resource(_) => None,
        }
    }
}

impl HttpRequest {
    pub fn validate(&self) -> Result<(), HttpContractError> {
        validate_target(&self.target)?;
        validate_headers(&self.headers)?;
        validate_body(&self.body, HTTP_MAXIMUM_REQUEST_BODY_BYTES)
            .map_err(|_| HttpContractError::RequestBodyOverflow)?;
        Ok(())
    }
}

impl HttpResponse {
    pub fn validate(&self) -> Result<(), HttpContractError> {
        if !(100..=599).contains(&self.status) {
            return Err(HttpContractError::InvalidStatus);
        }
        validate_headers(&self.headers)?;
        validate_body(&self.body, HTTP_MAXIMUM_RESPONSE_BODY_BYTES)
            .map_err(|_| HttpContractError::ResponseBodyOverflow)?;
        Ok(())
    }
}

fn validate_body(body: &HttpBody, maximum_inline_bytes: usize) -> Result<(), ()> {
    match body {
        HttpBody::InlineBytes(bytes) if bytes.as_slice().len() <= maximum_inline_bytes => Ok(()),
        HttpBody::InlineBytes(_) => Err(()),
        HttpBody::Resource(reference) => reference.validate().map_err(|_| ()),
    }
}

fn validate_target(target: &HttpTarget) -> Result<(), HttpContractError> {
    if target.authority().len() > HTTP_MAXIMUM_AUTHORITY_BYTES
        || target.path_and_query().len() > HTTP_MAXIMUM_TARGET_BYTES
        || !target.path_and_query().starts_with('/')
        || target
            .authority()
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b'/')
    {
        return Err(HttpContractError::InvalidTarget);
    }
    Ok(())
}

fn validate_headers(headers: &HttpHeaders) -> Result<(), HttpContractError> {
    if headers.len() > HTTP_MAXIMUM_HEADERS {
        return Err(HttpContractError::TooManyHeaders);
    }
    for header in headers.iter() {
        if header.name().is_empty() {
            return Err(HttpContractError::EmptyHeaderName);
        }
        if header.name().len() > HTTP_MAXIMUM_HEADER_NAME_BYTES {
            return Err(HttpContractError::HeaderNameOverflow);
        }
        if !header.name().bytes().all(is_header_name_byte) {
            return Err(HttpContractError::InvalidHeaderName);
        }
        if matches!(
            header.name().as_str(),
            "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
        ) {
            return Err(HttpContractError::SensitiveHeaderRequiresProtectedPath);
        }
        if matches!(
            header.name().as_str(),
            "content-length" | "transfer-encoding"
        ) {
            return Err(HttpContractError::FramingHeaderIsDerived);
        }
        if header.value().as_slice().len() > HTTP_MAXIMUM_HEADER_VALUE_BYTES {
            return Err(HttpContractError::HeaderValueOverflow);
        }
        if header
            .value()
            .as_slice()
            .iter()
            .any(|byte| *byte == b'\r' || *byte == b'\n')
        {
            return Err(HttpContractError::InvalidHeaderName);
        }
    }
    Ok(())
}

fn is_header_name_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase()
        || byte.is_ascii_digit()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

#[cfg(test)]
mod native_type_tests {
    use super::{
        HttpBody, HttpExchangeFailure, HttpHeader, HttpMethod, HttpRequest, HttpResponse,
        HttpScheme, HttpServerResponseRefusal, HttpTarget, HttpTransactionId,
    };
    use conduit_form::rust_binding::NativeRustBinding;

    fn assert_round_trip<T>(value: T)
    where
        T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
    {
        let structured = value
            .clone()
            .into_structured()
            .expect("native value encodes");
        assert_eq!(
            structured.value_type(),
            &T::semantic_type().expect("native semantic type checks")
        );
        assert_eq!(
            T::from_structured(structured).expect("native value decodes"),
            value
        );
    }

    #[test]
    fn http_method_and_terminal_families_are_native_semantic_types() {
        assert_round_trip(HttpMethod::Patch);
        assert_round_trip(HttpExchangeFailure::AuthorityDenied);
        assert_round_trip(HttpServerResponseRefusal::StaleTransaction);
    }

    #[test]
    fn portable_http_values_are_generated_from_conduitese() {
        assert_round_trip(HttpTransactionId::new(42).unwrap());
        assert_round_trip(
            HttpTarget::new(
                "api.example.test".into(),
                "/v1/items".into(),
                HttpScheme::Https,
            )
            .unwrap(),
        );
        let header = HttpHeader::new(
            "content-type".into(),
            conduit_form::rust_binding::BoundedBytes::new(b"application/json").unwrap(),
        )
        .unwrap();
        assert_round_trip(header.clone());
        let body = HttpBody::inline(b"{}".as_slice());
        assert_round_trip(body.clone());
        assert_round_trip(HttpRequest {
            transaction_id: HttpTransactionId::new(42).unwrap(),
            method: HttpMethod::Post,
            target: HttpTarget::new(
                "api.example.test".into(),
                "/v1/items".into(),
                HttpScheme::Https,
            )
            .unwrap(),
            headers: super::http_headers([header]).unwrap(),
            body: body.clone(),
        });
        assert_round_trip(HttpResponse {
            transaction_id: HttpTransactionId::new(42).unwrap(),
            status: 200,
            headers: Default::default(),
            body,
        });
    }
}
