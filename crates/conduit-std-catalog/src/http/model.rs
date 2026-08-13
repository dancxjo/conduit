use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub const HTTP_MAXIMUM_IN_FLIGHT: u16 = 4;
pub const HTTP_MAXIMUM_HEADERS: usize = 16;
pub const HTTP_MAXIMUM_HEADER_NAME_BYTES: usize = 64;
pub const HTTP_MAXIMUM_HEADER_VALUE_BYTES: usize = 512;
pub const HTTP_MAXIMUM_SCHEME_BYTES: usize = 8;
pub const HTTP_MAXIMUM_AUTHORITY_BYTES: usize = 255;
pub const HTTP_MAXIMUM_TARGET_BYTES: usize = 2_048;
pub const HTTP_MAXIMUM_REQUEST_BODY_BYTES: usize = 16_384;
pub const HTTP_MAXIMUM_RESPONSE_BODY_BYTES: usize = 65_536;
pub const HTTP_MAXIMUM_ENCODED_REQUEST_BYTES: u32 = 29_000;
pub const HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES: u32 = 78_000;
pub const HTTP_MAXIMUM_WORK_UNITS_PER_STEP: u16 = 1;
pub const HTTP_MANDATORY_SIGN_ITEMS_PER_TRANSACTION: u16 = 3;
pub const HTTP_AUTOMATIC_REDIRECTS: bool = false;
pub const HTTP_AUTOMATIC_RETRIES: bool = false;
pub const HTTP_AMBIENT_COOKIES: bool = false;
pub const HTTP_AMBIENT_CREDENTIALS: bool = false;
pub const HTTP_IMPLICIT_CACHING: bool = false;
pub const HTTP_IMPLICIT_DECOMPRESSION: bool = false;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpTransactionId(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpTarget {
    pub scheme: String,
    pub authority: String,
    pub path_and_query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpHeader {
    pub name: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequest {
    pub transaction_id: HttpTransactionId,
    pub method: HttpMethod,
    pub target: HttpTarget,
    /// Ordered and duplicate-preserving. Header names must be lowercase ASCII.
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponse {
    pub transaction_id: HttpTransactionId,
    /// HTTP status is exchange data, never a transport failure disposition.
    pub status: u16,
    /// Ordered and duplicate-preserving. Header names must be lowercase ASCII.
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpExchangeFailure {
    NameResolution,
    RouteUnavailable,
    Connect,
    Tls,
    ProviderLost,
    RequestOverflow,
    ResponseHeaderOverflow,
    ResponseBodyOverflow,
    Capacity,
    AuthorityDenied,
    Cancelled,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpServerResponseRefusal {
    UnknownTransaction,
    StaleTransaction,
    DuplicateResponse,
    LateResponse,
    ResponseHeaderOverflow,
    ResponseBodyOverflow,
    ListenerLost,
    Capacity,
    AuthorityDenied,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpContractError {
    EmptyAuthority,
    InvalidScheme,
    InvalidTarget,
    TooManyHeaders,
    EmptyHeaderName,
    InvalidHeaderName,
    SensitiveHeaderRequiresProtectedPath,
    FramingHeaderIsDerived,
    HeaderNameOverflow,
    HeaderValueOverflow,
    RequestBodyOverflow,
    ResponseBodyOverflow,
    InvalidStatus,
    MalformedEncoding,
    EncodedValueOverflow,
    TrailingBytes,
}

impl HttpRequest {
    pub fn validate(&self) -> Result<(), HttpContractError> {
        validate_target(&self.target)?;
        validate_headers(&self.headers)?;
        if self.body.len() > HTTP_MAXIMUM_REQUEST_BODY_BYTES {
            return Err(HttpContractError::RequestBodyOverflow);
        }
        Ok(())
    }
}

impl HttpResponse {
    pub fn validate(&self) -> Result<(), HttpContractError> {
        if !(100..=599).contains(&self.status) {
            return Err(HttpContractError::InvalidStatus);
        }
        validate_headers(&self.headers)?;
        if self.body.len() > HTTP_MAXIMUM_RESPONSE_BODY_BYTES {
            return Err(HttpContractError::ResponseBodyOverflow);
        }
        Ok(())
    }
}

fn validate_target(target: &HttpTarget) -> Result<(), HttpContractError> {
    if target.scheme != "http" && target.scheme != "https" {
        return Err(HttpContractError::InvalidScheme);
    }
    if target.scheme.len() > HTTP_MAXIMUM_SCHEME_BYTES {
        return Err(HttpContractError::InvalidScheme);
    }
    if target.authority.is_empty() {
        return Err(HttpContractError::EmptyAuthority);
    }
    if target.authority.len() > HTTP_MAXIMUM_AUTHORITY_BYTES
        || target.path_and_query.len() > HTTP_MAXIMUM_TARGET_BYTES
        || !target.path_and_query.starts_with('/')
        || target
            .authority
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
        if header.name.is_empty() {
            return Err(HttpContractError::EmptyHeaderName);
        }
        if header.name.len() > HTTP_MAXIMUM_HEADER_NAME_BYTES {
            return Err(HttpContractError::HeaderNameOverflow);
        }
        if !header.name.bytes().all(is_header_name_byte) {
            return Err(HttpContractError::InvalidHeaderName);
        }
        if matches!(
            header.name.as_str(),
            "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
        ) {
            return Err(HttpContractError::SensitiveHeaderRequiresProtectedPath);
        }
        if matches!(header.name.as_str(), "content-length" | "transfer-encoding") {
            return Err(HttpContractError::FramingHeaderIsDerived);
        }
        if header.value.len() > HTTP_MAXIMUM_HEADER_VALUE_BYTES {
            return Err(HttpContractError::HeaderValueOverflow);
        }
        if header
            .value
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
