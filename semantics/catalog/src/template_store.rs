//! Finite in-memory realization of the generic named-pattern storage contract.

use alloc::vec::Vec;
use conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES;

const SLOT_COUNT: usize = crate::MAXIMUM_NAMED_TEMPLATES as usize;

pub struct BoundedTemplateStore {
    command_prefix: Vec<u8>,
    result_prefix: Vec<u8>,
    slots: [TemplateSlot; SLOT_COUNT],
    output: Vec<u8>,
}

struct TemplateSlot {
    occupied: bool,
    name_len: usize,
    name: [u8; crate::MAXIMUM_TEMPLATE_NAME_BYTES],
    pattern_node: Vec<u8>,
}

impl TemplateSlot {
    fn prepare() -> Self {
        Self {
            occupied: false,
            name_len: 0,
            name: [0; crate::MAXIMUM_TEMPLATE_NAME_BYTES],
            pattern_node: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    fn name(&self) -> &[u8] {
        &self.name[..self.name_len]
    }
}

impl BoundedTemplateStore {
    pub fn prepare() -> Self {
        Self {
            command_prefix: crate::template_storage_command_type()
                .canonical_bytes()
                .expect("template command type is canonical"),
            result_prefix: crate::template_storage_result_type()
                .canonical_bytes()
                .expect("template result type is canonical"),
            slots: core::array::from_fn(|_| TemplateSlot::prepare()),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    pub fn execute(&mut self, input: &[u8]) -> Result<&[u8], TemplateStoreRefusal> {
        if input.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(TemplateStoreRefusal::Malformed);
        }
        let mut input = input
            .strip_prefix(self.command_prefix.as_slice())
            .ok_or(TemplateStoreRefusal::Malformed)?;
        if take_byte(&mut input)? != 3 {
            return Err(TemplateStoreRefusal::Malformed);
        }
        match take_bytes(&mut input)? {
            b"put" => self.put(input)?,
            b"get" => self.get(input)?,
            b"delete" => self.delete(input)?,
            _ => return Err(TemplateStoreRefusal::Malformed),
        }
        Ok(&self.output)
    }

    fn put(&mut self, mut input: &[u8]) -> Result<(), TemplateStoreRefusal> {
        if take_byte(&mut input)? != 2 || take_u32(&mut input)? != 2 {
            return Err(TemplateStoreRefusal::Malformed);
        }
        if take_bytes(&mut input)? != b"name" || take_byte(&mut input)? != 0 {
            return Err(TemplateStoreRefusal::Malformed);
        }
        let name = take_bytes(&mut input)?;
        validate_name(name)?;
        if take_bytes(&mut input)? != b"pattern" {
            return Err(TemplateStoreRefusal::Malformed);
        }
        let pattern_start = input;
        validate_pattern_node(&mut input)?;
        if !input.is_empty() {
            return Err(TemplateStoreRefusal::Malformed);
        }
        let pattern_len = pattern_start.len() - input.len();
        let pattern = &pattern_start[..pattern_len];
        if self
            .slots
            .iter()
            .any(|slot| slot.occupied && slot.name() == name)
        {
            return Err(TemplateStoreRefusal::DuplicateName);
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| !slot.occupied)
            .ok_or(TemplateStoreRefusal::Full)?;
        slot.occupied = true;
        slot.name_len = name.len();
        slot.name[..name.len()].copy_from_slice(name);
        slot.pattern_node.clear();
        slot.pattern_node.extend_from_slice(pattern);
        encode_name_result(&mut self.output, &self.result_prefix, b"stored", name);
        Ok(())
    }

    fn get(&mut self, mut input: &[u8]) -> Result<(), TemplateStoreRefusal> {
        let name = decode_name_payload(&mut input)?;
        if !input.is_empty() {
            return Err(TemplateStoreRefusal::Malformed);
        }
        let Some(slot) = self
            .slots
            .iter()
            .find(|slot| slot.occupied && slot.name() == name)
        else {
            encode_name_result(&mut self.output, &self.result_prefix, b"missing", name);
            return Ok(());
        };
        let mut retained = slot.pattern_node.as_slice();
        validate_pattern_node(&mut retained)
            .map_err(|_| TemplateStoreRefusal::CorruptRetainedTemplate)?;
        if !retained.is_empty() {
            return Err(TemplateStoreRefusal::CorruptRetainedTemplate);
        }
        self.output.clear();
        self.output.extend_from_slice(&self.result_prefix);
        self.output.push(3);
        bytes(&mut self.output, b"found");
        self.output.push(2);
        self.output.extend_from_slice(&2_u32.to_le_bytes());
        field_leaf(&mut self.output, b"name", name);
        bytes(&mut self.output, b"pattern");
        self.output.extend_from_slice(&slot.pattern_node);
        Ok(())
    }

    fn delete(&mut self, mut input: &[u8]) -> Result<(), TemplateStoreRefusal> {
        let name = decode_name_payload(&mut input)?;
        if !input.is_empty() {
            return Err(TemplateStoreRefusal::Malformed);
        }
        let tag = if let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.occupied && slot.name() == name)
        {
            slot.occupied = false;
            slot.name_len = 0;
            slot.pattern_node.clear();
            b"deleted".as_slice()
        } else {
            b"missing".as_slice()
        };
        encode_name_result(&mut self.output, &self.result_prefix, tag, name);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateStoreRefusal {
    Malformed,
    DuplicateName,
    Full,
    CorruptRetainedTemplate,
}

fn decode_name_payload<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], TemplateStoreRefusal> {
    if take_byte(input)? != 0 {
        return Err(TemplateStoreRefusal::Malformed);
    }
    let name = take_bytes(input)?;
    validate_name(name)?;
    Ok(name)
}

fn validate_name(name: &[u8]) -> Result<(), TemplateStoreRefusal> {
    if name.is_empty()
        || name.len() > crate::MAXIMUM_TEMPLATE_NAME_BYTES
        || core::str::from_utf8(name).is_err()
    {
        return Err(TemplateStoreRefusal::Malformed);
    }
    Ok(())
}

fn validate_pattern_node(input: &mut &[u8]) -> Result<(), TemplateStoreRefusal> {
    if take_byte(input)? != 2 || take_u32(input)? != 2 {
        return Err(TemplateStoreRefusal::Malformed);
    }
    if take_bytes(input)? != b"algorithm" || take_byte(input)? != 0 {
        return Err(TemplateStoreRefusal::Malformed);
    }
    if take_bytes(input)? != crate::NORMALIZATION_ALGORITHM.as_bytes() {
        return Err(TemplateStoreRefusal::Malformed);
    }
    if take_bytes(input)? != b"values" || take_byte(input)? != 0 {
        return Err(TemplateStoreRefusal::Malformed);
    }
    let values = take_bytes(input)?;
    if values.is_empty() {
        return Err(TemplateStoreRefusal::Malformed);
    }
    let mut count = 0;
    for raw in values.split(|byte| *byte == b',') {
        count += 1;
        if count >= crate::MAXIMUM_TIMED_EVENTS || parse_u64(raw)? > crate::NORMALIZED_SCALE {
            return Err(TemplateStoreRefusal::Malformed);
        }
    }
    Ok(())
}

fn parse_u64(raw: &[u8]) -> Result<u64, TemplateStoreRefusal> {
    if raw.is_empty() {
        return Err(TemplateStoreRefusal::Malformed);
    }
    raw.iter().try_fold(0_u64, |value, digit| {
        if !digit.is_ascii_digit() {
            return Err(TemplateStoreRefusal::Malformed);
        }
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(*digit - b'0')))
            .ok_or(TemplateStoreRefusal::Malformed)
    })
}

fn encode_name_result(output: &mut Vec<u8>, prefix: &[u8], tag: &[u8], name: &[u8]) {
    output.clear();
    output.extend_from_slice(prefix);
    output.push(3);
    bytes(output, tag);
    output.push(0);
    bytes(output, name);
}
fn field_leaf(output: &mut Vec<u8>, name: &[u8], value: &[u8]) {
    bytes(output, name);
    output.push(0);
    bytes(output, value);
}
fn bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}
fn take_byte(input: &mut &[u8]) -> Result<u8, TemplateStoreRefusal> {
    let (&value, rest) = input.split_first().ok_or(TemplateStoreRefusal::Malformed)?;
    *input = rest;
    Ok(value)
}
fn take_u32(input: &mut &[u8]) -> Result<u32, TemplateStoreRefusal> {
    let raw: [u8; 4] = input
        .get(..4)
        .ok_or(TemplateStoreRefusal::Malformed)?
        .try_into()
        .map_err(|_| TemplateStoreRefusal::Malformed)?;
    *input = &input[4..];
    Ok(u32::from_le_bytes(raw))
}
fn take_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], TemplateStoreRefusal> {
    let length = usize::try_from(take_u32(input)?).map_err(|_| TemplateStoreRefusal::Malformed)?;
    let value = input.get(..length).ok_or(TemplateStoreRefusal::Malformed)?;
    *input = &input[length..];
    Ok(value)
}

#[cfg(test)]
#[path = "template_store_tests.rs"]
mod tests;
