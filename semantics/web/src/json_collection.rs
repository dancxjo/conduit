//! One bounded, pure collection transition over canonical JSON meaning.
//!
//! The request has exactly `collection` (an array) and `command` (an object).
//! Array order is stable. Commands neither name a host nor perform storage.

use crate::{JsonCollectionRefusal, JsonValue};
use alloc::{string::String, vec::Vec};

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
        "append-unique" => {
            exact_fields(command, &["key", "op", "value"])?;
            let key = key_name(command)?;
            let value = field(command, "value")?;
            let key_value = object(value)
                .and_then(|fields| fields.iter().find(|(name, _)| name == key))
                .map(|(_, value)| value)
                .ok_or(JsonCollectionRefusal::MissingField)?;
            if keyed_position(&next, key, key_value)?.is_some() {
                return Err(JsonCollectionRefusal::InvalidCommand);
            }
            if next.len() == crate::JSON_MAXIMUM_ARRAY_ITEMS {
                return Err(JsonCollectionRefusal::CollectionFull);
            }
            next.push(value.clone());
        }
        "set-field-by-key" => {
            exact_fields(command, &["field", "key", "match", "op", "value"])?;
            let key = key_name(command)?;
            let target = field(command, "match")?;
            let position =
                keyed_position(&next, key, target)?.ok_or(JsonCollectionRefusal::MissingIndex)?;
            let JsonValue::String(name) = field(command, "field")? else {
                return Err(JsonCollectionRefusal::InvalidCommand);
            };
            if name.is_empty() || name == key {
                return Err(JsonCollectionRefusal::InvalidCommand);
            }
            let value = field(command, "value")?.clone();
            let JsonValue::Object(fields) = &mut next[position] else {
                return Err(JsonCollectionRefusal::InvalidCollection);
            };
            let (_, current) = fields
                .iter_mut()
                .find(|(field, _)| field == name)
                .ok_or(JsonCollectionRefusal::MissingField)?;
            *current = value;
        }
        "remove-by-key" => {
            exact_fields(command, &["key", "match", "op"])?;
            let position = keyed_position(&next, key_name(command)?, field(command, "match")?)?
                .ok_or(JsonCollectionRefusal::MissingIndex)?;
            next.remove(position);
        }
        _ => return Err(JsonCollectionRefusal::UnknownOperation),
    }
    let next = JsonValue::Array(next);
    next.validate()
        .map_err(JsonCollectionRefusal::InvalidValue)?;
    Ok(next)
}

fn key_name(command: &[(String, JsonValue)]) -> Result<&str, JsonCollectionRefusal> {
    let JsonValue::String(name) = field(command, "key")? else {
        return Err(JsonCollectionRefusal::InvalidCommand);
    };
    if name.is_empty() {
        return Err(JsonCollectionRefusal::InvalidCommand);
    }
    Ok(name)
}

/// Reject duplicate keys in the prior state so one command never arbitrarily
/// chooses among several records claiming the same identity.
fn keyed_position(
    items: &[JsonValue],
    key: &str,
    target: &JsonValue,
) -> Result<Option<usize>, JsonCollectionRefusal> {
    let mut position = None;
    for (index, item) in items.iter().enumerate() {
        let fields = object(item).ok_or(JsonCollectionRefusal::InvalidCollection)?;
        let value = fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
            .ok_or(JsonCollectionRefusal::MissingField)?;
        for previous in &items[..index] {
            let previous_fields =
                object(previous).ok_or(JsonCollectionRefusal::InvalidCollection)?;
            let previous_value = previous_fields
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
                .ok_or(JsonCollectionRefusal::MissingField)?;
            if previous_value == value {
                return Err(JsonCollectionRefusal::InvalidCollection);
            }
        }
        if value == target {
            position = Some(index);
        }
    }
    Ok(position)
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

/// Encoded entry point used by the ordinary admitted Host Call.
pub fn json_collection_step_bytes(input: &[u8]) -> Result<Vec<u8>, JsonCollectionRefusal> {
    let request = JsonValue::decode_info(input).map_err(JsonCollectionRefusal::InvalidValue)?;
    json_collection_step(&request)?
        .encode_info()
        .map_err(JsonCollectionRefusal::InvalidValue)
}
