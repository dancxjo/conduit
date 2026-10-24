//! Decode already-authenticated opaque IDs without inventing lifecycle authority.
//! serde's borrowed-string value deserializer does not support derived newtypes;
//! bounded JSON string framing preserves their ordinary Deserialize contract.
use serde::Deserialize;
pub fn identity<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, &'static str> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("face-scope-bound");
    }
    let mut framed = [b'"'; 66];
    framed[1..65].copy_from_slice(text.as_bytes());
    serde_json::from_slice(&framed).map_err(|_| "face-scope")
}
