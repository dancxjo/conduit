//! One bounded, pure collection transition over canonical JSON meaning.
//!
//! The request has exactly `collection` (an array) and `command` (an object).
//! Array order is stable. Commands neither name a Host nor perform storage.

use crate::{JsonRefusal, JsonValue};
use alloc::{string::String, vec::Vec};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u16)]
pub enum JsonCollectionRefusal {
    InvalidRequest = 100,
    InvalidCollection = 101,
    InvalidCommand = 102,
    UnknownOperation = 103,
    InvalidIndex = 104,
    MissingIndex = 105,
    MissingField = 106,
    NotBoolean = 107,
    CollectionFull = 108,
    InvalidValue(JsonRefusal) = 109,
}

/// Produces a new validated collection or a machine-distinct refusal. The
/// caller's prior collection is unchanged on both success and failure.
pub fn json_collection_step(request: &JsonValue) -> Result<JsonValue, JsonCollectionRefusal> {
    request
        .validate()
        .map_err(JsonCollectionRefusal::InvalidValue)?;
    let request = object(request).ok_or(JsonCollectionRefusal::InvalidRequest)?;
    exact_fields(request, &["collection", "command"])?;
    let JsonValue::Array(prior) = field(request, "collection")? else {
        return Err(JsonCollectionRefusal::InvalidCollection);
    };
    let command =
        object(field(request, "command")?).ok_or(JsonCollectionRefusal::InvalidCommand)?;
    let JsonValue::String(operation) = field(command, "op")? else {
        return Err(JsonCollectionRefusal::InvalidCommand);
    };
    let mut next = prior.clone();
    match operation.as_str() {
        "append" => {
            exact_fields(command, &["op", "value"])?;
            if next.len() == crate::JSON_MAXIMUM_ARRAY_ITEMS {
                return Err(JsonCollectionRefusal::CollectionFull);
            }
            next.push(field(command, "value")?.clone());
        }
        "replace" => {
            exact_fields(command, &["index", "op", "value"])?;
            let index = index(command, next.len())?;
            next[index] = field(command, "value")?.clone();
        }
        "remove" => {
            exact_fields(command, &["index", "op"])?;
            let index = index(command, next.len())?;
            next.remove(index);
        }
        "toggle" => {
            exact_fields(command, &["field", "index", "op"])?;
            let index = index(command, next.len())?;
            let JsonValue::String(name) = field(command, "field")? else {
                return Err(JsonCollectionRefusal::InvalidCommand);
            };
            let JsonValue::Object(members) = &mut next[index] else {
                return Err(JsonCollectionRefusal::InvalidCollection);
            };
            let (_, value) = members
                .iter_mut()
                .find(|(key, _)| key == name)
                .ok_or(JsonCollectionRefusal::MissingField)?;
            let JsonValue::Bool(value) = value else {
                return Err(JsonCollectionRefusal::NotBoolean);
            };
            *value = !*value;
        }
        "clear" => {
            exact_fields(command, &["op"])?;
            next.clear();
        }
        _ => return Err(JsonCollectionRefusal::UnknownOperation),
    }
    let next = JsonValue::Array(next);
    next.validate()
        .map_err(JsonCollectionRefusal::InvalidValue)?;
    Ok(next)
}

fn object(value: &JsonValue) -> Option<&[(String, JsonValue)]> {
    if let JsonValue::Object(fields) = value {
        Some(fields)
    } else {
        None
    }
}

fn field<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
) -> Result<&'a JsonValue, JsonCollectionRefusal> {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
        .ok_or(JsonCollectionRefusal::InvalidCommand)
}

fn exact_fields(
    fields: &[(String, JsonValue)],
    expected: &[&str],
) -> Result<(), JsonCollectionRefusal> {
    if fields.len() != expected.len()
        || fields
            .iter()
            .zip(expected)
            .any(|((key, _), expected)| key != expected)
    {
        return Err(JsonCollectionRefusal::InvalidCommand);
    }
    Ok(())
}

fn index(command: &[(String, JsonValue)], length: usize) -> Result<usize, JsonCollectionRefusal> {
    let JsonValue::Number(number) = field(command, "index")? else {
        return Err(JsonCollectionRefusal::InvalidIndex);
    };
    let raw = number.raw_microunits();
    if raw < 0 || raw % 1_000_000 != 0 {
        return Err(JsonCollectionRefusal::InvalidIndex);
    }
    let index =
        usize::try_from(raw / 1_000_000).map_err(|_| JsonCollectionRefusal::InvalidIndex)?;
    if index >= length {
        return Err(JsonCollectionRefusal::MissingIndex);
    }
    Ok(index)
}

/// Encoded entry point used by the ordinary admitted Host operation.
pub fn json_collection_step_bytes(input: &[u8]) -> Result<Vec<u8>, JsonCollectionRefusal> {
    let request = JsonValue::decode_info(input).map_err(JsonCollectionRefusal::InvalidValue)?;
    json_collection_step(&request)?
        .encode_info()
        .map_err(JsonCollectionRefusal::InvalidValue)
}
