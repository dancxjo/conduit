//! Portable finite named collections of normalized patterns.

use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
    StructuredInfoValueShape,
};

pub const MAXIMUM_NAMED_TEMPLATES: u16 = 8;
pub const MAXIMUM_TEMPLATE_NAME_BYTES: usize = 64;
pub const TEMPLATE_COLLECTION_SCHEMA: &str = "sequence/named-pattern-template-collection@1";
pub const TEMPLATE_SLOT_SCHEMA: &str = "sequence/named-pattern-template-slot@1";
pub const TEMPLATE_NAME_INFO_ID: &str = "sequence/pattern-template-name@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateCollectionRefusal {
    Malformed,
    NameEmpty,
    NameTooLong,
    DuplicateName,
    CollectionFull,
    NotFound,
    CorruptTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodedSlot {
    active: bool,
    name: String,
    pattern: StructuredInfoValue,
}

pub fn named_pattern_template_slot_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(TEMPLATE_SLOT_SCHEMA),
        vec![
            field_type(
                "active",
                StructuredInfoType::leaf(kind_id("value/boolean@1")).unwrap(),
            ),
            field_type(
                "name",
                StructuredInfoType::leaf(kind_id(TEMPLATE_NAME_INFO_ID)).unwrap(),
            ),
            field_type("pattern", crate::normalized_duration_sequence_type()),
        ],
    )
    .unwrap()
}

pub fn named_pattern_template_collection_type() -> StructuredInfoType {
    StructuredInfoType::collection(
        named_pattern_template_slot_type(),
        Some(MAXIMUM_NAMED_TEMPLATES),
    )
    .unwrap()
}

pub fn empty_named_pattern_template_collection() -> StructuredInfoValue {
    let placeholder = crate::normalized_value(&[1]).expect("placeholder normalized pattern");
    let slots = (0..MAXIMUM_NAMED_TEMPLATES)
        .map(|_| slot_value(false, "", placeholder.clone()).expect("inactive template slot"))
        .collect();
    StructuredInfoValue::collection(named_pattern_template_collection_type(), slots)
        .expect("fixed bounded template collection")
}

pub fn insert_named_pattern_template(
    collection: &StructuredInfoValue,
    name: &str,
    pattern: &StructuredInfoValue,
) -> Result<StructuredInfoValue, TemplateCollectionRefusal> {
    validate_name(name)?;
    validate_pattern(pattern)?;
    let mut slots = decode_collection(collection)?;
    if slots.iter().any(|slot| slot.active && slot.name == name) {
        return Err(TemplateCollectionRefusal::DuplicateName);
    }
    let slot = slots
        .iter_mut()
        .find(|slot| !slot.active)
        .ok_or(TemplateCollectionRefusal::CollectionFull)?;
    *slot = DecodedSlot {
        active: true,
        name: name.into(),
        pattern: pattern.clone(),
    };
    encode_collection(slots)
}

pub fn lookup_named_pattern_template(
    collection: &StructuredInfoValue,
    name: &str,
) -> Result<StructuredInfoValue, TemplateCollectionRefusal> {
    validate_name(name)?;
    decode_collection(collection)?
        .into_iter()
        .find(|slot| slot.active && slot.name == name)
        .map(|slot| slot.pattern)
        .ok_or(TemplateCollectionRefusal::NotFound)
}

pub fn remove_named_pattern_template(
    collection: &StructuredInfoValue,
    name: &str,
) -> Result<StructuredInfoValue, TemplateCollectionRefusal> {
    validate_name(name)?;
    let mut slots = decode_collection(collection)?;
    let slot = slots
        .iter_mut()
        .find(|slot| slot.active && slot.name == name)
        .ok_or(TemplateCollectionRefusal::NotFound)?;
    slot.active = false;
    slot.name.clear();
    slot.pattern =
        crate::normalized_value(&[1]).map_err(|_| TemplateCollectionRefusal::Malformed)?;
    encode_collection(slots)
}

fn decode_collection(
    collection: &StructuredInfoValue,
) -> Result<Vec<DecodedSlot>, TemplateCollectionRefusal> {
    if collection.value_type() != &named_pattern_template_collection_type() {
        return Err(TemplateCollectionRefusal::Malformed);
    }
    let values = match collection.shape() {
        StructuredInfoValueShape::Collection(values)
            if values.len() == usize::from(MAXIMUM_NAMED_TEMPLATES) =>
        {
            values
        }
        _ => return Err(TemplateCollectionRefusal::Malformed),
    };
    let slots = values
        .iter()
        .map(decode_slot)
        .collect::<Result<Vec<_>, _>>()?;
    for (index, left) in slots.iter().enumerate() {
        if left.active
            && slots[index + 1..]
                .iter()
                .any(|right| right.active && right.name == left.name)
        {
            return Err(TemplateCollectionRefusal::DuplicateName);
        }
    }
    Ok(slots)
}

fn decode_slot(value: &StructuredInfoValue) -> Result<DecodedSlot, TemplateCollectionRefusal> {
    if value.value_type() != &named_pattern_template_slot_type() {
        return Err(TemplateCollectionRefusal::Malformed);
    }
    let fields = match value.shape() {
        StructuredInfoValueShape::Record(fields) => fields,
        _ => return Err(TemplateCollectionRefusal::Malformed),
    };
    let active = match leaf(field(fields, "active")?)? {
        b"true" => true,
        b"false" => false,
        _ => return Err(TemplateCollectionRefusal::Malformed),
    };
    let name = core::str::from_utf8(leaf(field(fields, "name")?)?)
        .map_err(|_| TemplateCollectionRefusal::Malformed)?;
    let pattern = field(fields, "pattern")?.clone();
    if active {
        validate_name(name)?;
        validate_pattern(&pattern)?;
    } else if !name.is_empty() {
        return Err(TemplateCollectionRefusal::Malformed);
    }
    Ok(DecodedSlot {
        active,
        name: name.into(),
        pattern,
    })
}

fn validate_pattern(pattern: &StructuredInfoValue) -> Result<(), TemplateCollectionRefusal> {
    if pattern.value_type() != &crate::normalized_duration_sequence_type() {
        return Err(TemplateCollectionRefusal::CorruptTemplate);
    }
    crate::compare_normalized_patterns(pattern, pattern, crate::MAXIMUM_ABSOLUTE_METRIC, 0)
        .map(|_| ())
        .map_err(|_| TemplateCollectionRefusal::CorruptTemplate)
}

fn validate_name(name: &str) -> Result<(), TemplateCollectionRefusal> {
    if name.is_empty() {
        return Err(TemplateCollectionRefusal::NameEmpty);
    }
    if name.len() > MAXIMUM_TEMPLATE_NAME_BYTES {
        return Err(TemplateCollectionRefusal::NameTooLong);
    }
    Ok(())
}

fn encode_collection(
    slots: Vec<DecodedSlot>,
) -> Result<StructuredInfoValue, TemplateCollectionRefusal> {
    let values = slots
        .into_iter()
        .map(|slot| slot_value(slot.active, &slot.name, slot.pattern))
        .collect::<Result<Vec<_>, _>>()?;
    StructuredInfoValue::collection(named_pattern_template_collection_type(), values)
        .map_err(|_| TemplateCollectionRefusal::Malformed)
}

fn slot_value(
    active: bool,
    name: &str,
    pattern: StructuredInfoValue,
) -> Result<StructuredInfoValue, TemplateCollectionRefusal> {
    StructuredInfoValue::record(
        named_pattern_template_slot_type(),
        vec![
            leaf_field(
                "active",
                "value/boolean@1",
                if active { "true" } else { "false" },
            )?,
            leaf_field("name", TEMPLATE_NAME_INFO_ID, name)?,
            StructuredFieldValue::new("pattern", pattern)
                .map_err(|_| TemplateCollectionRefusal::Malformed)?,
        ],
    )
    .map_err(|_| TemplateCollectionRefusal::Malformed)
}

fn field_type(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).unwrap()
}

fn leaf_field(
    name: &str,
    kind: &str,
    value: &str,
) -> Result<StructuredFieldValue, TemplateCollectionRefusal> {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(kind)).unwrap(),
            value.as_bytes().to_vec(),
        )
        .map_err(|_| TemplateCollectionRefusal::Malformed)?,
    )
    .map_err(|_| TemplateCollectionRefusal::Malformed)
}

fn field<'a>(
    fields: &'a [StructuredFieldValue],
    name: &str,
) -> Result<&'a StructuredInfoValue, TemplateCollectionRefusal> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(TemplateCollectionRefusal::Malformed)
}

fn leaf(value: &StructuredInfoValue) -> Result<&[u8], TemplateCollectionRefusal> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(value) => Ok(value),
        _ => Err(TemplateCollectionRefusal::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_named_collection_inserts_looks_up_and_removes_exact_template() {
        let empty = empty_named_pattern_template_collection();
        let pattern = crate::normalized_value(&[250_000, 1_000_000, 500_000]).unwrap();
        let stored = insert_named_pattern_template(&empty, "front-door", &pattern).unwrap();
        assert_eq!(
            lookup_named_pattern_template(&stored, "front-door").unwrap(),
            pattern
        );
        let removed = remove_named_pattern_template(&stored, "front-door").unwrap();
        assert_eq!(
            lookup_named_pattern_template(&removed, "front-door"),
            Err(TemplateCollectionRefusal::NotFound)
        );
    }

    #[test]
    fn duplicate_full_missing_and_invalid_names_remain_distinct() {
        let pattern = crate::normalized_value(&[1]).unwrap();
        let mut collection = empty_named_pattern_template_collection();
        collection = insert_named_pattern_template(&collection, "one", &pattern).unwrap();
        assert_eq!(
            insert_named_pattern_template(&collection, "one", &pattern),
            Err(TemplateCollectionRefusal::DuplicateName)
        );
        for index in 1..MAXIMUM_NAMED_TEMPLATES {
            collection = insert_named_pattern_template(
                &collection,
                &alloc::format!("slot-{index}"),
                &pattern,
            )
            .unwrap();
        }
        assert_eq!(
            insert_named_pattern_template(&collection, "overflow", &pattern),
            Err(TemplateCollectionRefusal::CollectionFull)
        );
        assert_eq!(
            lookup_named_pattern_template(&collection, "missing"),
            Err(TemplateCollectionRefusal::NotFound)
        );
        assert_eq!(
            lookup_named_pattern_template(&collection, ""),
            Err(TemplateCollectionRefusal::NameEmpty)
        );
        assert_eq!(
            lookup_named_pattern_template(
                &collection,
                &"x".repeat(MAXIMUM_TEMPLATE_NAME_BYTES + 1)
            ),
            Err(TemplateCollectionRefusal::NameTooLong)
        );
    }
}
