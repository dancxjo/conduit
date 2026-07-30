use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

const MAGIC: &[u8] = b"CND\x01";
const HASH_DOMAIN: &[u8] = b"conduit.semantic-hash/v1\0";
const MAX_DEPTH: usize = 64;

fn validate_identifier(val: &str) -> Result<Vec<u8>, String> {
    if val.is_empty() || val.ends_with('.') || val.ends_with('/') {
        return Err(format!("invalid identifier: {val:?}"));
    }
    // Match ASCII identifier pattern: ^[a-z](?:[a-z0-9_-]|[.](?=[a-z])|/(?!.*[/])(?=[a-z]))*$
    let bytes = val.as_bytes();
    if !bytes[0].is_ascii_lowercase() {
        return Err(format!("invalid identifier: {val:?}"));
    }
    let mut i = 0;
    let len = bytes.len();
    while i < len {
        let b = bytes[i];
        if b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-' {
            i += 1;
        } else if b == b'.' {
            if i + 1 >= len || !bytes[i + 1].is_ascii_lowercase() {
                return Err(format!("invalid identifier: {val:?}"));
            }
            i += 1;
        } else if b == b'/' {
            if i + 1 >= len || !bytes[i + 1].is_ascii_lowercase() {
                return Err(format!("invalid identifier: {val:?}"));
            }
            // Check that there is no second slash
            if bytes[i + 1..].contains(&b'/') {
                return Err(format!("invalid identifier: {val:?}"));
            }
            i += 1;
        } else {
            return Err(format!("invalid identifier: {val:?}"));
        }
    }

    let mut out = vec![0x22];
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(out)
}

fn encode_canonical(value: &Value, depth: usize) -> Result<Vec<u8>, String> {
    if value.is_null() {
        return Ok(vec![0x00]);
    }
    let obj = value
        .as_object()
        .ok_or_else(|| format!("value must be a single-tag object or null: {value:?}"))?;
    if obj.len() != 1 {
        return Err(format!(
            "value must be a single-tag object or null: {value:?}"
        ));
    }
    let (tag, payload) = obj.iter().next().unwrap();
    match tag.as_str() {
        "boolean" => {
            let b = payload.as_bool().ok_or("boolean payload must be bool")?;
            Ok(vec![if b { 0x02 } else { 0x01 }])
        }
        "integer" => {
            let num: i128 = if let Some(i) = payload.as_i64() {
                i as i128
            } else if let Some(u) = payload.as_u64() {
                u as i128
            } else if let Some(s) = payload.as_str() {
                s.parse::<i128>().map_err(|e| e.to_string())?
            } else {
                return Err(format!("invalid integer payload: {payload:?}"));
            };
            let mut out = vec![0x10];
            out.extend_from_slice(&num.to_be_bytes());
            Ok(out)
        }
        "bytes" => {
            let hex_str = payload.as_str().ok_or("bytes payload must be string")?;
            let raw = hex::decode(hex_str).map_err(|e| e.to_string())?;
            let mut out = vec![0x20];
            out.extend_from_slice(&(raw.len() as u64).to_be_bytes());
            out.extend(raw);
            Ok(out)
        }
        "text" => {
            let txt = payload.as_str().ok_or("text payload must be string")?;
            let bytes = txt.as_bytes();
            let mut out = vec![0x21];
            out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
            out.extend_from_slice(bytes);
            Ok(out)
        }
        "identifier" => {
            let id_str = payload
                .as_str()
                .ok_or("identifier payload must be string")?;
            validate_identifier(id_str)
        }
        "list" => {
            let new_depth = depth + 1;
            if new_depth > MAX_DEPTH {
                return Err("canonical value nesting exceeds 64".to_string());
            }
            let arr = payload.as_array().ok_or("list payload must be array")?;
            let mut members = Vec::new();
            for item in arr {
                members.extend(encode_canonical(item, new_depth)?);
            }
            let mut out = vec![0x30];
            out.extend_from_slice(&(arr.len() as u64).to_be_bytes());
            out.extend(members);
            Ok(out)
        }
        "set" => {
            let new_depth = depth + 1;
            if new_depth > MAX_DEPTH {
                return Err("canonical value nesting exceeds 64".to_string());
            }
            let arr = payload.as_array().ok_or("set payload must be array")?;
            let mut encoded_members = Vec::new();
            for item in arr {
                encoded_members.push(encode_canonical(item, new_depth)?);
            }
            encoded_members.sort();
            for i in 1..encoded_members.len() {
                if encoded_members[i] == encoded_members[i - 1] {
                    return Err("duplicate canonical set value".to_string());
                }
            }
            let mut out = vec![0x32];
            out.extend_from_slice(&(arr.len() as u64).to_be_bytes());
            for m in encoded_members {
                out.extend(m);
            }
            Ok(out)
        }
        "map" => {
            let new_depth = depth + 1;
            if new_depth > MAX_DEPTH {
                return Err("canonical value nesting exceeds 64".to_string());
            }
            let fields = payload.as_array().ok_or("map payload must be array")?;
            let mut names = BTreeSet::new();
            let mut entries: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
            for field in fields {
                let name = field
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("map field missing name")?;
                if !names.insert(name.to_string()) {
                    return Err(format!("duplicate canonical map key: {name}"));
                }
                let key_enc = validate_identifier(name)?;
                let disposition = field
                    .get("disposition")
                    .and_then(Value::as_str)
                    .unwrap_or("semantic");
                if disposition == "annotation" {
                    continue;
                }
                let val = field.get("value").ok_or("map field missing value")?;
                let encoded_val = encode_canonical(val, new_depth)?;
                if disposition == "defaulted" {
                    let default_val = field
                        .get("default")
                        .ok_or("defaulted field missing default")?;
                    if encoded_val == encode_canonical(default_val, new_depth)? {
                        continue;
                    }
                } else if disposition != "semantic" {
                    return Err(format!("unknown disposition: {disposition}"));
                }
                entries.push((key_enc, encoded_val));
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = vec![0x31];
            out.extend_from_slice(&(entries.len() as u64).to_be_bytes());
            for (k, v) in entries {
                out.extend(k);
                out.extend(v);
            }
            Ok(out)
        }
        _ => Err(format!("unknown canonical value tag: {tag}")),
    }
}

fn descriptor_bytes(kind: &str, schema_version: u32, body: &Value) -> Result<Vec<u8>, String> {
    let mut out = MAGIC.to_vec();
    out.extend(validate_identifier(kind)?);
    out.extend_from_slice(&schema_version.to_be_bytes());
    out.extend(encode_canonical(body, 0)?);
    Ok(out)
}

struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self.offset + len;
        if end > self.data.len() {
            return Err("truncated canonical value".to_string());
        }
        let res = &self.data[self.offset..end];
        self.offset = end;
        Ok(res)
    }

    fn length(&mut self) -> Result<usize, String> {
        let bytes = self.take(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(bytes);
        Ok(u64::from_be_bytes(arr) as usize)
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        let tag = self.take(1)?[0];
        match tag {
            0x00 => Ok(Value::Null),
            0x01 => Ok(serde_json::json!({ "boolean": false })),
            0x02 => Ok(serde_json::json!({ "boolean": true })),
            0x10 => {
                let bytes = self.take(16)?;
                let mut arr = [0u8; 16];
                arr.copy_from_slice(bytes);
                let num = i128::from_be_bytes(arr);
                Ok(serde_json::json!({ "integer": num }))
            }
            0x20 => {
                let len = self.length()?;
                let raw = self.take(len)?;
                Ok(serde_json::json!({ "bytes": hex::encode(raw) }))
            }
            0x21 => {
                let len = self.length()?;
                let raw = self.take(len)?;
                let text = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
                Ok(serde_json::json!({ "text": text }))
            }
            0x22 => {
                let len = self.length()?;
                let raw = self.take(len)?;
                let id = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
                validate_identifier(id)?;
                Ok(serde_json::json!({ "identifier": id }))
            }
            0x30 => {
                let new_depth = depth + 1;
                if new_depth > MAX_DEPTH {
                    return Err("canonical value nesting exceeds 64".to_string());
                }
                let count = self.length()?;
                let mut list = Vec::with_capacity(count);
                for _ in 0..count {
                    list.push(self.value(new_depth)?);
                }
                Ok(serde_json::json!({ "list": list }))
            }
            0x31 => {
                let new_depth = depth + 1;
                if new_depth > MAX_DEPTH {
                    return Err("canonical value nesting exceeds 64".to_string());
                }
                let count = self.length()?;
                let mut fields = Vec::with_capacity(count);
                let mut previous: Option<Vec<u8>> = None;
                for _ in 0..count {
                    let key_start = self.offset;
                    let key = self.value(new_depth)?;
                    let key_id = key
                        .get("identifier")
                        .and_then(Value::as_str)
                        .ok_or("canonical map key is not an identifier")?;
                    let key_bytes = self.data[key_start..self.offset].to_vec();
                    if let Some(ref prev) = previous {
                        if key_bytes <= *prev {
                            return Err("canonical map keys are not strictly ordered".to_string());
                        }
                    }
                    previous = Some(key_bytes);
                    let val = self.value(new_depth)?;
                    fields.push(serde_json::json!({
                        "name": key_id,
                        "value": val
                    }));
                }
                Ok(serde_json::json!({ "map": fields }))
            }
            0x32 => {
                let new_depth = depth + 1;
                if new_depth > MAX_DEPTH {
                    return Err("canonical value nesting exceeds 64".to_string());
                }
                let count = self.length()?;
                let mut members = Vec::with_capacity(count);
                let mut previous: Option<Vec<u8>> = None;
                for _ in 0..count {
                    let member_start = self.offset;
                    let m = self.value(new_depth)?;
                    let member_bytes = self.data[member_start..self.offset].to_vec();
                    if let Some(ref prev) = previous {
                        if member_bytes <= *prev {
                            return Err(
                                "canonical set members are not strictly ordered".to_string()
                            );
                        }
                    }
                    previous = Some(member_bytes);
                    members.push(m);
                }
                Ok(serde_json::json!({ "set": members }))
            }
            tag => Err(format!("unknown canonical tag: 0x{tag:02x}")),
        }
    }

    fn descriptor(&mut self) -> Result<(String, u32, Value), String> {
        if self.take(MAGIC.len())? != MAGIC {
            return Err("not a canonical descriptor v1".to_string());
        }
        let kind_val = self.value(0)?;
        let kind = kind_val
            .get("identifier")
            .and_then(Value::as_str)
            .ok_or("descriptor kind is not an identifier")?
            .to_string();

        let ver_bytes = self.take(4)?;
        let mut ver_arr = [0u8; 4];
        ver_arr.copy_from_slice(ver_bytes);
        let schema_version = u32::from_be_bytes(ver_arr);

        let body = self.value(0)?;
        if self.offset != self.data.len() {
            return Err("trailing canonical descriptor bytes".to_string());
        }
        Ok((kind, schema_version, body))
    }
}

pub fn run(
    workspace_root: &Path,
    vectors_file: Option<PathBuf>,
    show: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path =
        vectors_file.unwrap_or_else(|| workspace_root.join("conformance/c1/canonical-v1.json"));
    if !path.exists() {
        return Err(format!("Vectors file missing: {}", path.display()).into());
    }

    let text = fs::read_to_string(&path)?;
    let suite: Value = serde_json::from_str(&text)?;

    if suite.get("canonical_form_version").and_then(Value::as_u64) != Some(1) {
        return Err("reader only supports canonical form version 1".into());
    }

    let vectors = suite
        .get("vectors")
        .and_then(Value::as_array)
        .ok_or("vectors missing or not array")?;

    for vector in vectors {
        let name = vector.get("name").and_then(Value::as_str).unwrap_or("");
        let kind = vector.get("kind").and_then(Value::as_str).unwrap_or("");
        let schema_version = vector
            .get("schema_version")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        let body = vector.get("body").ok_or("vector missing body")?;

        let canonical =
            descriptor_bytes(kind, schema_version, body).map_err(|e| format!("{name}: {e}"))?;

        let mut hasher = Sha256::new();
        hasher.update(HASH_DOMAIN);
        hasher.update(&canonical);
        let digest = format!("sha256:{:x}", hasher.finalize());

        let (decoded_kind, decoded_version, decoded_body) = Reader::new(&canonical)
            .descriptor()
            .map_err(|e| format!("{name}: decode error: {e}"))?;

        let re_canonical = descriptor_bytes(&decoded_kind, decoded_version, &decoded_body)
            .map_err(|e| format!("{name}: re-encode error: {e}"))?;

        if re_canonical != canonical {
            return Err(format!("{name}: decoded value did not round-trip").into());
        }

        if let Some(equivs) = vector.get("equivalent_bodies").and_then(Value::as_array) {
            for equiv in equivs {
                let alt = descriptor_bytes(kind, schema_version, equiv)
                    .map_err(|e| format!("{name}: equiv error: {e}"))?;
                if alt != canonical {
                    return Err(format!("{name}: equivalent input changed canonical bytes").into());
                }
            }
        }

        if let Some(diffs) = vector.get("different_bodies").and_then(Value::as_array) {
            for diff in diffs {
                let alt = descriptor_bytes(kind, schema_version, diff)
                    .map_err(|e| format!("{name}: diff error: {e}"))?;
                if alt == canonical {
                    return Err(format!("{name}: semantic change retained canonical bytes").into());
                }
                let mut alt_hasher = Sha256::new();
                alt_hasher.update(HASH_DOMAIN);
                alt_hasher.update(&alt);
                if alt_hasher.finalize() == Sha256::digest([HASH_DOMAIN, &canonical].concat()) {
                    return Err(format!("{name}: semantic change retained semantic hash").into());
                }
            }
        }

        if show {
            println!("{name}");
            println!("canonical_hex={}", hex::encode(&canonical));
            println!("semantic_hash={digest}");
        } else {
            let expected_hex = vector
                .get("canonical_hex")
                .and_then(Value::as_str)
                .unwrap_or("");
            let expected_hash = vector
                .get("semantic_hash")
                .and_then(Value::as_str)
                .unwrap_or("");
            if hex::encode(&canonical) != expected_hex {
                return Err(format!("{name}: canonical bytes differ").into());
            }
            if digest != expected_hash {
                return Err(format!("{name}: semantic hash differs").into());
            }
            println!("ok {name} {digest}");
        }
    }

    if let Some(negatives) = suite.get("negative_vectors").and_then(Value::as_array) {
        for vector in negatives {
            let name = vector.get("name").and_then(Value::as_str).unwrap_or("");
            let kind = vector.get("kind").and_then(Value::as_str).unwrap_or("");
            let schema_version = vector
                .get("schema_version")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            let body = vector.get("body").ok_or("negative vector missing body")?;
            let expected_error = vector
                .get("expected_error")
                .and_then(Value::as_str)
                .unwrap_or("");

            let actual = match descriptor_bytes(kind, schema_version, body) {
                Err(e) => {
                    if e.starts_with("duplicate canonical map key") {
                        "duplicate-map-key"
                    } else if e.starts_with("invalid identifier") {
                        "invalid-identifier"
                    } else if e == "canonical value nesting exceeds 64" {
                        "maximum-depth-exceeded"
                    } else {
                        "malformed-canonical-value"
                    }
                }
                Ok(_) => "accepted",
            };

            if actual != expected_error {
                return Err(format!("{name}: expected {expected_error}, got {actual}").into());
            }
            println!("ok {name} rejected {actual}");
        }
    }

    Ok(())
}
