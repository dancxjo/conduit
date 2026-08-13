use super::{
    HttpContractError, HttpHeader, HttpMethod, HttpRequest, HttpResponse, HttpTarget,
    HttpTransactionId, HTTP_MAXIMUM_ENCODED_REQUEST_BYTES, HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
};
use alloc::string::String;
use alloc::vec::Vec;

const REQUEST_TAG: u8 = 1;
const RESPONSE_TAG: u8 = 2;

pub fn encode_request(value: &HttpRequest) -> Result<Vec<u8>, HttpContractError> {
    value.validate()?;
    let mut out = Vec::new();
    out.push(REQUEST_TAG);
    out.extend_from_slice(&value.transaction_id.0.to_be_bytes());
    out.push(method_tag(value.method));
    put_bytes(&mut out, value.target.scheme.as_bytes())?;
    put_bytes(&mut out, value.target.authority.as_bytes())?;
    put_bytes(&mut out, value.target.path_and_query.as_bytes())?;
    put_headers(&mut out, &value.headers)?;
    put_bytes(&mut out, &value.body)?;
    if out.len() > HTTP_MAXIMUM_ENCODED_REQUEST_BYTES as usize {
        return Err(HttpContractError::EncodedValueOverflow);
    }
    Ok(out)
}

pub fn decode_request(encoded: &[u8]) -> Result<HttpRequest, HttpContractError> {
    if encoded.len() > HTTP_MAXIMUM_ENCODED_REQUEST_BYTES as usize {
        return Err(HttpContractError::EncodedValueOverflow);
    }
    let mut input = Input::new(encoded);
    if input.byte()? != REQUEST_TAG {
        return Err(HttpContractError::MalformedEncoding);
    }
    let transaction_id = HttpTransactionId(input.u64()?);
    let method = method(input.byte()?)?;
    let target = HttpTarget {
        scheme: input.string()?,
        authority: input.string()?,
        path_and_query: input.string()?,
    };
    let headers = input.headers()?;
    let body = input.bytes()?.to_vec();
    input.finish()?;
    let value = HttpRequest {
        transaction_id,
        method,
        target,
        headers,
        body,
    };
    value.validate()?;
    Ok(value)
}

pub fn encode_response(value: &HttpResponse) -> Result<Vec<u8>, HttpContractError> {
    value.validate()?;
    let mut out = Vec::new();
    out.push(RESPONSE_TAG);
    out.extend_from_slice(&value.transaction_id.0.to_be_bytes());
    out.extend_from_slice(&value.status.to_be_bytes());
    put_headers(&mut out, &value.headers)?;
    put_bytes(&mut out, &value.body)?;
    if out.len() > HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES as usize {
        return Err(HttpContractError::EncodedValueOverflow);
    }
    Ok(out)
}

pub fn decode_response(encoded: &[u8]) -> Result<HttpResponse, HttpContractError> {
    if encoded.len() > HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES as usize {
        return Err(HttpContractError::EncodedValueOverflow);
    }
    let mut input = Input::new(encoded);
    if input.byte()? != RESPONSE_TAG {
        return Err(HttpContractError::MalformedEncoding);
    }
    let transaction_id = HttpTransactionId(input.u64()?);
    let status = input.u16()?;
    let headers = input.headers()?;
    let body = input.bytes()?.to_vec();
    input.finish()?;
    let value = HttpResponse {
        transaction_id,
        status,
        headers,
        body,
    };
    value.validate()?;
    Ok(value)
}

fn method_tag(method: HttpMethod) -> u8 {
    match method {
        HttpMethod::Get => 0,
        HttpMethod::Head => 1,
        HttpMethod::Post => 2,
        HttpMethod::Put => 3,
        HttpMethod::Patch => 4,
        HttpMethod::Delete => 5,
        HttpMethod::Options => 6,
    }
}
fn method(tag: u8) -> Result<HttpMethod, HttpContractError> {
    match tag {
        0 => Ok(HttpMethod::Get),
        1 => Ok(HttpMethod::Head),
        2 => Ok(HttpMethod::Post),
        3 => Ok(HttpMethod::Put),
        4 => Ok(HttpMethod::Patch),
        5 => Ok(HttpMethod::Delete),
        6 => Ok(HttpMethod::Options),
        _ => Err(HttpContractError::MalformedEncoding),
    }
}

fn put_headers(out: &mut Vec<u8>, headers: &[HttpHeader]) -> Result<(), HttpContractError> {
    let count =
        u16::try_from(headers.len()).map_err(|_| HttpContractError::EncodedValueOverflow)?;
    out.extend_from_slice(&count.to_be_bytes());
    for header in headers {
        put_bytes(out, header.name.as_bytes())?;
        put_bytes(out, &header.value)?;
    }
    Ok(())
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), HttpContractError> {
    let length = u32::try_from(bytes.len()).map_err(|_| HttpContractError::EncodedValueOverflow)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

struct Input<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Input<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], HttpContractError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(HttpContractError::MalformedEncoding)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(HttpContractError::MalformedEncoding)?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, HttpContractError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, HttpContractError> {
        Ok(u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| HttpContractError::MalformedEncoding)?,
        ))
    }
    fn u32(&mut self) -> Result<u32, HttpContractError> {
        Ok(u32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| HttpContractError::MalformedEncoding)?,
        ))
    }
    fn u64(&mut self) -> Result<u64, HttpContractError> {
        Ok(u64::from_be_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| HttpContractError::MalformedEncoding)?,
        ))
    }
    fn bytes(&mut self) -> Result<&'a [u8], HttpContractError> {
        let length =
            usize::try_from(self.u32()?).map_err(|_| HttpContractError::MalformedEncoding)?;
        self.take(length)
    }
    fn string(&mut self) -> Result<String, HttpContractError> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| HttpContractError::MalformedEncoding)
    }
    fn headers(&mut self) -> Result<Vec<HttpHeader>, HttpContractError> {
        let count = usize::from(self.u16()?);
        let mut headers = Vec::with_capacity(count);
        for _ in 0..count {
            headers.push(HttpHeader {
                name: self.string()?,
                value: self.bytes()?.to_vec(),
            });
        }
        Ok(headers)
    }
    fn finish(self) -> Result<(), HttpContractError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(HttpContractError::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

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
            body: b"bounded".to_vec(),
        }
    }

    #[test]
    fn request_round_trip_preserves_order_and_duplicates() {
        let value = request();
        assert_eq!(
            decode_request(&encode_request(&value).unwrap()).unwrap(),
            value
        );
    }
    #[test]
    fn response_status_is_data_including_500() {
        let value = HttpResponse {
            transaction_id: HttpTransactionId(42),
            status: 500,
            headers: Vec::new(),
            body: b"error document".to_vec(),
        };
        assert_eq!(
            decode_response(&encode_response(&value).unwrap()).unwrap(),
            value
        );
    }
    #[test]
    fn malformed_and_trailing_framing_refuse_distinctly() {
        assert_eq!(
            decode_request(&[REQUEST_TAG]),
            Err(HttpContractError::MalformedEncoding)
        );
        let mut encoded = encode_request(&request()).unwrap();
        encoded.push(0);
        assert_eq!(
            decode_request(&encoded),
            Err(HttpContractError::TrailingBytes)
        );
    }
    #[test]
    fn invalid_headers_and_body_overflow_refuse() {
        let mut value = request();
        value.headers[0].name = "Upper".into();
        assert_eq!(value.validate(), Err(HttpContractError::InvalidHeaderName));
        value = request();
        value
            .body
            .resize(super::super::HTTP_MAXIMUM_REQUEST_BODY_BYTES + 1, 0);
        assert_eq!(
            value.validate(),
            Err(HttpContractError::RequestBodyOverflow)
        );
    }

    #[test]
    fn credentials_and_cookies_cannot_enter_the_ordinary_header_path() {
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
    }

    #[test]
    fn framing_is_derived_from_the_exact_bounded_body() {
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
