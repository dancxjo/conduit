//! Finite text-level address detection independent of speech or model machinery.

use alloc::{string::String, vec::Vec};

use crate::MAX_TEXT_BYTES;

pub const MAX_ADDRESS_NAMES: usize = 8;
pub const MAX_ADDRESS_NAME_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressSet {
    names: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddressConfigurationError {
    Empty,
    TooManyNames,
    EmptyName,
    NameTooLarge,
    InvalidName,
    DuplicateName,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddressDetectionError {
    RecognizedTextTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddressDetection {
    NotAddressed,
    Addressed {
        matched_name_index: u8,
        utterance: String,
    },
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
                .any(|prior: &String| prior.eq_ignore_ascii_case(name))
            {
                return Err(AddressConfigurationError::DuplicateName);
            }
            admitted.push(String::from(*name));
        }
        Ok(Self { names: admitted })
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn detect(&self, recognized: &str) -> Result<AddressDetection, AddressDetectionError> {
        if recognized.len() > MAX_TEXT_BYTES as usize {
            return Err(AddressDetectionError::RecognizedTextTooLarge);
        }
        let candidate = recognized.trim_start();
        for (index, name) in self.names.iter().enumerate() {
            let Some(consumed) = matching_prefix_bytes(candidate, name) else {
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
            return Ok(AddressDetection::Addressed {
                matched_name_index: index as u8,
                utterance,
            });
        }
        Ok(AddressDetection::NotAddressed)
    }
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
