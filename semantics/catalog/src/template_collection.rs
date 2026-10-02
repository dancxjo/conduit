//! Portable finite named collections of normalized patterns.

use alloc::{string::String, vec::Vec};
use conduit_core::{StructuredInfoType, StructuredInfoValue};
use conduit_plot::rust_binding::NativeRustBinding;
pub use conduit_time::TemplateCollectionRefusal;

pub const MAXIMUM_NAMED_TEMPLATES: u16 = 8;
pub const MAXIMUM_TEMPLATE_NAME_BYTES: usize = 64;
pub const TEMPLATE_COLLECTION_SCHEMA: &str = "sequence/named-pattern-template-collection@1";

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodedSlot {
    active: bool,
    name: String,
    pattern: StructuredInfoValue,
}

pub fn named_pattern_template_slot_type() -> StructuredInfoType {
    conduit_time::NamedPatternTemplateSlot::semantic_type()
        .expect("checked named pattern template slot Type")
}

pub fn named_pattern_template_collection_type() -> StructuredInfoType {
    conduit_time::NamedPatternTemplateSlots::semantic_type()
        .expect("checked named pattern template slots Type")
}

pub fn empty_named_pattern_template_collection() -> StructuredInfoValue {
    let placeholder = crate::normalized_value(&[1]).expect("placeholder normalized pattern");
    let slots = (0..MAXIMUM_NAMED_TEMPLATES)
        .map(|_| DecodedSlot {
            active: false,
            name: String::new(),
            pattern: placeholder.clone(),
        })
        .collect();
    encode_collection(slots).expect("fixed bounded template collection")
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
    let native = conduit_time::NamedPatternTemplateSlots::from_structured(collection.clone())
        .map_err(|_| TemplateCollectionRefusal::Malformed)?;
    let slots = native
        .get()
        .iter()
        .map(|slot| {
            Ok(DecodedSlot {
                active: *slot.active(),
                name: slot.name().clone(),
                pattern: slot
                    .pattern()
                    .clone()
                    .into_structured()
                    .map_err(|_| TemplateCollectionRefusal::Malformed)?,
            })
        })
        .collect::<Result<Vec<_>, TemplateCollectionRefusal>>()?;
    for slot in &slots {
        if slot.active {
            validate_name(&slot.name)?;
            validate_pattern(&slot.pattern)?;
        } else if !slot.name.is_empty() {
            return Err(TemplateCollectionRefusal::Malformed);
        }
    }
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
        .map(|slot| {
            let pattern = conduit_time::NormalizedDurationSequence::from_structured(slot.pattern)
                .map_err(|_| TemplateCollectionRefusal::Malformed)?;
            conduit_time::NamedPatternTemplateSlot::new(slot.active, slot.name, pattern)
                .map_err(|_| TemplateCollectionRefusal::Malformed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let values: [conduit_time::NamedPatternTemplateSlot; 8] = values
        .try_into()
        .map_err(|_| TemplateCollectionRefusal::Malformed)?;
    conduit_time::NamedPatternTemplateSlots::new(values)
        .map_err(|_| TemplateCollectionRefusal::Malformed)?
        .into_structured()
        .map_err(|_| TemplateCollectionRefusal::Malformed)
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
