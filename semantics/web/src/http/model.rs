use alloc::vec::Vec;
use conduit_core::BoundedResourceRef;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    HttpContractError, HttpExchangeFailure, HttpHeader, HttpMethod, HttpScheme,
    HttpServerResponseRefusal, HttpTarget, HttpTransactionId,
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

impl HttpScheme {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpBody {
    Inline(Vec<u8>),
    Resource(BoundedResourceRef),
}

impl HttpBody {
    pub fn inline(bytes: impl Into<Vec<u8>>) -> Self {
        Self::Inline(bytes.into())
    }

    pub fn as_inline(&self) -> Option<&[u8]> {
        match self {
            Self::Inline(bytes) => Some(bytes),
            Self::Resource(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub transaction_id: HttpTransactionId,
    pub method: HttpMethod,
    pub target: HttpTarget,
    /// Ordered and duplicate-preserving. Header names must be lowercase ASCII.
    pub headers: Vec<HttpHeader>,
    pub body: HttpBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub transaction_id: HttpTransactionId,
    /// HTTP status is exchange data, never a transport failure disposition.
    pub status: u16,
    /// Ordered and duplicate-preserving. Header names must be lowercase ASCII.
    pub headers: Vec<HttpHeader>,
    pub body: HttpBody,
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
        HttpBody::Inline(bytes) if bytes.len() <= maximum_inline_bytes => Ok(()),
        HttpBody::Inline(_) => Err(()),
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

fn validate_headers(headers: &[HttpHeader]) -> Result<(), HttpContractError> {
    if headers.len() > HTTP_MAXIMUM_HEADERS {
        return Err(HttpContractError::TooManyHeaders);
    }
    for header in headers {
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
        HttpExchangeFailure, HttpHeader, HttpMethod, HttpScheme, HttpServerResponseRefusal,
        HttpTarget, HttpTransactionId,
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
        assert_round_trip(
            HttpHeader::new(
                "content-type".into(),
                conduit_form::rust_binding::BoundedBytes::new(b"application/json").unwrap(),
            )
            .unwrap(),
        );
    }
}
