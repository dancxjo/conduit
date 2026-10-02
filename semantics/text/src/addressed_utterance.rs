//! Finite text-level address detection independent of speech or model machinery.

use alloc::{string::String, vec::Vec};
use conduit_plot::rust_binding::BoundedSequence;
use serde::{Deserialize, Serialize};

use crate::{
    AddressConfigurationError, AddressDetection, AddressDetectionRefusal, AddressName,
    AddressNames, AddressSet, AddressValueError, MAX_TEXT_BYTES,
};

pub const MAX_ADDRESS_NAMES: usize = 8;
pub const MAX_ADDRESS_NAME_BYTES: usize = 64;
pub const MAX_ADDRESS_SET_VALUE_BYTES: usize = 1_024;
pub const MAX_ADDRESS_DETECTION_VALUE_BYTES: usize = 1_024;

const ADDRESS_SET_SCHEMA: &str = "conduit.text/address-set-value@1";
const ADDRESS_DETECTION_SCHEMA: &str = "conduit.text/address-detection-value@1";

#[derive(Serialize, Deserialize)]
struct AddressSetValue {
    schema: String,
    names: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct AddressDetectionValue {
    schema: String,
    status: String,
    matched_name_index: Option<u8>,
    utterance: Option<String>,
}

impl AddressSet {
    pub fn new(names: &[&str]) -> Result<Self, AddressConfigurationError> {
        if names.is_empty() {
            return Err(AddressConfigurationError::Empty);
        }
        if names.len() > MAX_ADDRESS_NAMES {
            return Err(AddressConfigurationError::TooManyNames);
        }
        let mut admitted = Vec::with_capacity(names.len());
        for name in names {
            if name.is_empty() {
                return Err(AddressConfigurationError::EmptyName);
            }
            if name.len() > MAX_ADDRESS_NAME_BYTES {
                return Err(AddressConfigurationError::NameTooLarge);
            }
            if name.trim() != *name
                || name
                    .chars()
                    .any(|character| character.is_control() || matches!(character, ',' | ':' | ';'))
            {
                return Err(AddressConfigurationError::InvalidName);
            }
            if admitted
                .iter()
                .any(|prior: &AddressName| prior.get().eq_ignore_ascii_case(name))
            {
                return Err(AddressConfigurationError::DuplicateName);
            }
            admitted.push(
                AddressName::new(String::from(*name))
                    .map_err(|_| AddressConfigurationError::NameTooLarge)?,
            );
        }
        let admitted = BoundedSequence::try_from_iter(admitted)
            .map_err(|_| AddressConfigurationError::TooManyNames)?;
        Self::new_native(
            AddressNames::new(admitted).map_err(|_| AddressConfigurationError::TooManyNames)?,
        )
        .map_err(|_| AddressConfigurationError::Empty)
    }

    pub fn detect(&self, recognized: &str) -> Result<AddressDetection, AddressDetectionRefusal> {
        if recognized.len() > MAX_TEXT_BYTES as usize {
            return Err(AddressDetectionRefusal::RecognizedTextTooLarge);
        }
        let candidate = recognized.trim_start();
        for (index, name) in self.names().get().iter().enumerate() {
            let Some(consumed) = matching_prefix_bytes(candidate, name.get()) else {
                continue;
            };
            let remainder = &candidate[consumed..];
            if remainder
                .chars()
                .next()
                .is_some_and(|character| !is_address_boundary(character))
            {
                continue;
            }
            let utterance = String::from(remainder.trim_start_matches(|character: char| {
                character.is_whitespace() || matches!(character, ',' | ':' | ';' | '-' | '—')
            }));
            return Ok(AddressDetection::addressed(index as u8, utterance)
                .expect("address detection already checked native bounds"));
        }
        Ok(AddressDetection::NotAddressed)
    }
}

pub fn encode_address_set(addresses: &AddressSet) -> Result<Vec<u8>, AddressValueError> {
    encode_bounded(
        &AddressSetValue {
            schema: ADDRESS_SET_SCHEMA.into(),
            names: addresses
                .names()
                .get()
                .iter()
                .map(|name| name.get().clone())
                .collect(),
        },
        MAX_ADDRESS_SET_VALUE_BYTES,
    )
}

pub fn decode_address_set(bytes: &[u8]) -> Result<AddressSet, AddressValueError> {
    let value: AddressSetValue = decode_canonical(bytes, MAX_ADDRESS_SET_VALUE_BYTES)?;
    if value.schema != ADDRESS_SET_SCHEMA {
        return Err(AddressValueError::InvalidValue);
    }
    let names = value.names.iter().map(String::as_str).collect::<Vec<_>>();
    AddressSet::new(&names).map_err(|_| AddressValueError::InvalidValue)
}

pub fn encode_address_detection(
    detection: &AddressDetection,
) -> Result<Vec<u8>, AddressValueError> {
    let (status, matched_name_index, utterance) = match detection {
        AddressDetection::NotAddressed => ("not-addressed", None, None),
        AddressDetection::Addressed(payload) => (
            "addressed",
            Some(*payload.matched_name_index()),
            Some(payload.utterance().clone()),
        ),
    };
    encode_bounded(
        &AddressDetectionValue {
            schema: ADDRESS_DETECTION_SCHEMA.into(),
            status: status.into(),
            matched_name_index,
            utterance,
        },
        MAX_ADDRESS_DETECTION_VALUE_BYTES,
    )
}

pub fn decode_address_detection(bytes: &[u8]) -> Result<AddressDetection, AddressValueError> {
    let value: AddressDetectionValue = decode_canonical(bytes, MAX_ADDRESS_DETECTION_VALUE_BYTES)?;
    if value.schema != ADDRESS_DETECTION_SCHEMA {
        return Err(AddressValueError::InvalidValue);
    }
    match (
        value.status.as_str(),
        value.matched_name_index,
        value.utterance,
    ) {
        ("not-addressed", None, None) => Ok(AddressDetection::NotAddressed),
        ("addressed", Some(matched_name_index), Some(utterance))
            if usize::from(matched_name_index) < MAX_ADDRESS_NAMES
                && utterance.len() <= MAX_TEXT_BYTES as usize =>
        {
            AddressDetection::addressed(matched_name_index, utterance)
                .map_err(|_| AddressValueError::InvalidValue)
        }
        _ => Err(AddressValueError::InvalidValue),
    }
}

fn encode_bounded<T: Serialize>(value: &T, maximum: usize) -> Result<Vec<u8>, AddressValueError> {
    let bytes = serde_json::to_vec(value).map_err(|_| AddressValueError::Malformed)?;
    if bytes.len() > maximum {
        return Err(AddressValueError::BoundExceeded);
    }
    Ok(bytes)
}

fn decode_canonical<T>(bytes: &[u8], maximum: usize) -> Result<T, AddressValueError>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    if bytes.len() > maximum {
        return Err(AddressValueError::BoundExceeded);
    }
    let value = serde_json::from_slice(bytes).map_err(|_| AddressValueError::Malformed)?;
    if encode_bounded(&value, maximum)? != bytes {
        return Err(AddressValueError::NonCanonical);
    }
    Ok(value)
}

fn matching_prefix_bytes(text: &str, name: &str) -> Option<usize> {
    let mut text_characters = text.char_indices();
    let mut consumed = 0;
    for expected in name.chars() {
        let (offset, actual) = text_characters.next()?;
        if actual != expected && !actual.eq_ignore_ascii_case(&expected) {
            return None;
        }
        consumed = offset + actual.len_utf8();
    }
    Some(consumed)
}

fn is_address_boundary(character: char) -> bool {
    character.is_whitespace() || matches!(character, ',' | ':' | ';' | '-' | '—' | '.' | '!' | '?')
}
