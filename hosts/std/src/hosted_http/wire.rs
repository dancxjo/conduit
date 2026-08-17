use conduit_std_catalog::{
    HttpBody, HttpExchangeFailure, HttpHeader, HttpMethod, HttpRequest, HttpResponse, HttpTarget,
    HttpTransactionId, HTTP_MAXIMUM_HEADERS, HTTP_MAXIMUM_HEADER_NAME_BYTES,
    HTTP_MAXIMUM_HEADER_VALUE_BYTES, HTTP_MAXIMUM_REQUEST_BODY_BYTES,
    HTTP_MAXIMUM_RESPONSE_BODY_BYTES,
};
use std::io::Read;

const MAXIMUM_HEAD_BYTES: usize = HTTP_MAXIMUM_HEADERS
    * (HTTP_MAXIMUM_HEADER_NAME_BYTES + HTTP_MAXIMUM_HEADER_VALUE_BYTES + 4)
    + 4_096;

pub(super) fn encode_request(request: &HttpRequest) -> Result<Vec<u8>, HttpExchangeFailure> {
    let body = inline_body(&request.body)?;
    let mut out = Vec::with_capacity(MAXIMUM_HEAD_BYTES.min(8_192) + body.len());
    out.extend_from_slice(method(request.method));
    out.push(b' ');
    out.extend_from_slice(request.target.path_and_query.as_bytes());
    out.extend_from_slice(b" HTTP/1.1\r\nHost: ");
    out.extend_from_slice(request.target.authority.as_bytes());
    out.extend_from_slice(b"\r\nConnection: close\r\nAccept-Encoding: identity\r\n");
    for header in &request.headers {
        out.extend_from_slice(header.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(&header.value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

pub(super) fn encode_response(response: &HttpResponse) -> Result<Vec<u8>, HttpExchangeFailure> {
    let body = inline_body(&response.body)?;
    let mut out = Vec::with_capacity(MAXIMUM_HEAD_BYTES.min(8_192) + body.len());
    out.extend_from_slice(format!("HTTP/1.1 {} Conduit\r\n", response.status).as_bytes());
    out.extend_from_slice(b"Connection: close\r\n");
    for header in &response.headers {
        out.extend_from_slice(header.name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(&header.value);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

pub(super) fn read_response(
    reader: &mut impl Read,
    transaction_id: HttpTransactionId,
) -> Result<HttpResponse, HttpExchangeFailure> {
    let (start, headers, body) = read_message(
        reader,
        HTTP_MAXIMUM_RESPONSE_BODY_BYTES,
        HttpExchangeFailure::ResponseBodyOverflow,
    )?;
    let status = start
        .split_ascii_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| (100..=599).contains(value))
        .ok_or(HttpExchangeFailure::ProviderLost)?;
    Ok(HttpResponse {
        transaction_id,
        status,
        headers,
        body: HttpBody::inline(body),
    })
}

pub(super) fn read_request(
    reader: &mut impl Read,
    transaction_id: HttpTransactionId,
) -> Result<HttpRequest, HttpExchangeFailure> {
    let (start, headers, body) = read_message(
        reader,
        HTTP_MAXIMUM_REQUEST_BODY_BYTES,
        HttpExchangeFailure::RequestOverflow,
    )?;
    let mut fields = start.split_ascii_whitespace();
    let method = parse_method(fields.next().ok_or(HttpExchangeFailure::ProviderLost)?)?;
    let path_and_query = fields
        .next()
        .filter(|value| value.starts_with('/'))
        .ok_or(HttpExchangeFailure::ProviderLost)?
        .to_string();
    let authority = headers
        .iter()
        .find(|header| header.name == "host")
        .and_then(|header| String::from_utf8(header.value.clone()).ok())
        .ok_or(HttpExchangeFailure::ProviderLost)?;
    let semantic_headers = headers
        .into_iter()
        .filter(|header| {
            !matches!(
                header.name.as_str(),
                "host" | "connection" | "accept-encoding"
            )
        })
        .collect();
    Ok(HttpRequest {
        transaction_id,
        method,
        target: HttpTarget {
            scheme: "http".into(),
            authority,
            path_and_query,
        },
        headers: semantic_headers,
        body: HttpBody::inline(body),
    })
}

fn inline_body(body: &HttpBody) -> Result<&[u8], HttpExchangeFailure> {
    body.as_inline()
        .ok_or(HttpExchangeFailure::ResourceUnavailable)
}

fn read_message(
    reader: &mut impl Read,
    maximum_body: usize,
    body_overflow: HttpExchangeFailure,
) -> Result<(String, Vec<HttpHeader>, Vec<u8>), HttpExchangeFailure> {
    let mut bytes = Vec::with_capacity(MAXIMUM_HEAD_BYTES.min(8_192));
    let head_end = loop {
        if bytes.len() == MAXIMUM_HEAD_BYTES {
            return Err(HttpExchangeFailure::ResponseHeaderOverflow);
        }
        let mut byte = [0_u8; 1];
        reader
            .read_exact(&mut byte)
            .map_err(|_| HttpExchangeFailure::ProviderLost)?;
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len() - 4;
        }
    };
    let head =
        std::str::from_utf8(&bytes[..head_end]).map_err(|_| HttpExchangeFailure::ProviderLost)?;
    let mut lines = head.split("\r\n");
    let start = lines
        .next()
        .filter(|line| !line.is_empty())
        .ok_or(HttpExchangeFailure::ProviderLost)?
        .to_string();
    let mut headers = Vec::with_capacity(HTTP_MAXIMUM_HEADERS);
    let mut content_length = None;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or(HttpExchangeFailure::ProviderLost)?;
        let name = name.to_ascii_lowercase();
        let value = value.trim_ascii().as_bytes().to_vec();
        if headers.len() == HTTP_MAXIMUM_HEADERS
            || name.len() > HTTP_MAXIMUM_HEADER_NAME_BYTES
            || value.len() > HTTP_MAXIMUM_HEADER_VALUE_BYTES
        {
            return Err(HttpExchangeFailure::ResponseHeaderOverflow);
        }
        if name == "transfer-encoding" || name == "content-encoding" {
            return Err(HttpExchangeFailure::ProviderLost);
        }
        if name == "content-length" {
            content_length = std::str::from_utf8(&value)
                .ok()
                .and_then(|text| text.parse::<usize>().ok());
            if content_length.is_none() {
                return Err(HttpExchangeFailure::ProviderLost);
            }
            continue;
        }
        headers.push(HttpHeader { name, value });
    }
    let body_length = content_length.ok_or(HttpExchangeFailure::ProviderLost)?;
    if body_length > maximum_body {
        return Err(body_overflow);
    }
    let mut body = vec![0; body_length];
    reader
        .read_exact(&mut body)
        .map_err(|_| HttpExchangeFailure::ProviderLost)?;
    Ok((start, headers, body))
}

fn method(method: HttpMethod) -> &'static [u8] {
    match method {
        HttpMethod::Get => b"GET",
        HttpMethod::Head => b"HEAD",
        HttpMethod::Post => b"POST",
        HttpMethod::Put => b"PUT",
        HttpMethod::Patch => b"PATCH",
        HttpMethod::Delete => b"DELETE",
        HttpMethod::Options => b"OPTIONS",
    }
}

fn parse_method(method: &str) -> Result<HttpMethod, HttpExchangeFailure> {
    match method {
        "GET" => Ok(HttpMethod::Get),
        "HEAD" => Ok(HttpMethod::Head),
        "POST" => Ok(HttpMethod::Post),
        "PUT" => Ok(HttpMethod::Put),
        "PATCH" => Ok(HttpMethod::Patch),
        "DELETE" => Ok(HttpMethod::Delete),
        "OPTIONS" => Ok(HttpMethod::Options),
        _ => Err(HttpExchangeFailure::ProviderLost),
    }
}
