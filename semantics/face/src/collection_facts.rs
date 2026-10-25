//! Canonical collection facts checked before structural region selection.
//! Salvaged from held Source 97277a8; this validates borrowed facts only and
//! does not create collection interactions, paging state, or action authority.
use crate::{
    ChoiceMultiplicity, Face, FaceAction, FacePropertyValue, FaceRelationshipKind, FaceRole,
    FaceSubject,
};
use alloc::{collections::BTreeSet, vec::Vec};

const MAX_PREPARED_COLLECTION_ITEMS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionFactsRefusal {
    MissingCollection,
    MissingFact,
    AmbiguousFact,
    Type,
    Capacity,
    Membership,
    Order,
    Selection,
    Action,
    UnsupportedProfile,
}
use CollectionFactsRefusal as Refusal;

pub(crate) fn property<'a>(
    face: &'a Face,
    subject: &str,
    name: &str,
) -> Result<&'a FacePropertyValue, Refusal> {
    optional_property(face, subject, name)?.ok_or(Refusal::MissingFact)
}
fn optional_property<'a>(
    face: &'a Face,
    subject: &str,
    name: &str,
) -> Result<Option<&'a FacePropertyValue>, Refusal> {
    let mut values = face
        .properties
        .iter()
        .filter(|p| p.subject == subject && p.name == name);
    let first = values.next().map(|p| &p.value);
    if values.next().is_some() {
        return Err(Refusal::AmbiguousFact);
    }
    Ok(first)
}
fn count(face: &Face, subject: &str, name: &str) -> Result<u64, Refusal> {
    match property(face, subject, name)? {
        FacePropertyValue::Count(n) => Ok(*n),
        _ => Err(Refusal::Type),
    }
}
fn identity<'a>(face: &'a Face, subject: &str, name: &str) -> Result<&'a str, Refusal> {
    match property(face, subject, name)? {
        FacePropertyValue::Identity(id) => Ok(id),
        _ => Err(Refusal::Type),
    }
}
fn selected(face: &Face, subject: &str) -> Result<bool, Refusal> {
    match property(face, subject, "selected")? {
        FacePropertyValue::Flag(value) => Ok(*value),
        _ => Err(Refusal::Type),
    }
}
fn value<'a>(
    face: &'a Face,
    subject: &str,
    name: &str,
) -> Result<(&'a conduit_core::CheckedValueContract, &'a [u8]), Refusal> {
    match property(face, subject, name)? {
        FacePropertyValue::TypedValue { contract, bytes } => Ok((contract, bytes)),
        _ => Err(Refusal::Type),
    }
}
pub(crate) fn multiplicity(
    face: &Face,
    subject: &str,
) -> Result<Option<ChoiceMultiplicity>, Refusal> {
    match optional_property(face, subject, "choice-multiplicity")? {
        None => Ok(None),
        Some(FacePropertyValue::Text(mode)) if mode == "exclusive" => {
            Ok(Some(ChoiceMultiplicity::Exclusive))
        }
        Some(FacePropertyValue::Text(mode)) if mode == "independent" => {
            Ok(Some(ChoiceMultiplicity::Independent))
        }
        _ => Err(Refusal::UnsupportedProfile),
    }
}

pub(crate) fn validate_collection(
    face: &Face,
    collection: &str,
    maximum_items: usize,
) -> Result<(), Refusal> {
    if maximum_items == 0 || maximum_items > MAX_PREPARED_COLLECTION_ITEMS {
        return Err(Refusal::Capacity);
    }
    face.subjects
        .iter()
        .find(|s| s.identity == collection && s.role == FaceRole::Collection)
        .ok_or(Refusal::MissingCollection)?;
    let declared = count(face, collection, "item-count")?;
    if declared > maximum_items as u64 {
        return Err(Refusal::Capacity);
    }
    let mut items = Vec::with_capacity(declared as usize);
    let mut identities = BTreeSet::new();
    let mut orders = BTreeSet::new();
    for relationship in face
        .relationships
        .iter()
        .filter(|r| r.source == collection && r.kind == FaceRelationshipKind::Contains)
    {
        if items.len() >= declared as usize || !identities.insert(&relationship.target) {
            return Err(Refusal::Membership);
        }
        let item = face
            .subjects
            .iter()
            .find(|s| s.identity == relationship.target)
            .ok_or(Refusal::Membership)?;
        let order = count(face, &item.identity, "order")?;
        if !orders.insert(order) {
            return Err(Refusal::Order);
        }
        items.push(item);
    }
    if items.len() != declared as usize {
        return Err(Refusal::Membership);
    }
    items.sort_by_key(|item| count(face, &item.identity, "order").expect("admitted source order"));
    let multiplicity = multiplicity(face, collection)?;
    match multiplicity {
        Some(ChoiceMultiplicity::Exclusive) => {
            exclusive(face, collection, &items)?;
        }
        Some(ChoiceMultiplicity::Independent) => {
            independent_counts(face, collection, &items)?;
            for item in &items {
                independent(face, &item.identity)?;
            }
        }
        None => {}
    }
    Ok(())
}

fn exclusive<'a>(
    face: &'a Face,
    collection: &str,
    items: &[&FaceSubject],
) -> Result<&'a FaceAction, Refusal> {
    if count(face, collection, "minimum-selected-count")? != 1
        || count(face, collection, "maximum-selected-count")? != 1
    {
        return Err(Refusal::UnsupportedProfile);
    }
    let action_id = identity(face, collection, "selection-action")?;
    let source = identity(face, collection, "value-source")?;
    if identity(face, source, "value-change-action")? != action_id {
        return Err(Refusal::Action);
    }
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == action_id && a.target == source && a.arguments.len() == 1)
        .ok_or(Refusal::Action)?;
    let (contract, current) = value(face, source, "current-value")?;
    if contract != &action.arguments[0].contract {
        return Err(Refusal::Action);
    }
    let mut values = BTreeSet::new();
    let mut selected_count = 0;
    for item in items {
        let (item_contract, bytes) = value(face, &item.identity, "choice-value")?;
        if item_contract.value_kind != contract.value_kind
            || contract.validate(bytes).is_err()
            || !values.insert(bytes)
        {
            return Err(Refusal::Membership);
        }
        let chosen = selected(face, &item.identity)?;
        if chosen != (bytes == current) {
            return Err(Refusal::Selection);
        }
        selected_count += usize::from(chosen);
    }
    if selected_count != 1 {
        return Err(Refusal::Selection);
    }
    // This profile promises all exact alternatives, not an implicit filtered
    // subset. A positive finite membership bounds the domain; validate every
    // member against the complete contract so intersecting constraints retain
    // their ordinary checked meaning.
    let members = contract
        .constraints
        .iter()
        .find_map(|constraint| match constraint {
            conduit_core::ValueConstraint::CanonicalMembership {
                members,
                negated: false,
            } => Some(members),
            _ => None,
        })
        .ok_or(Refusal::UnsupportedProfile)?;
    for member in members
        .iter()
        .filter(|member| contract.validate(member).is_ok())
    {
        if !values.contains(member.as_slice()) {
            return Err(Refusal::Membership);
        }
    }
    Ok(action)
}
// This profile admits unconstrained Boolean replacements only. Inventory facts
// describe the full source inventory; a query may expose only a subset of it.
fn independent_counts(
    face: &Face,
    collection: &str,
    items: &[&FaceSubject],
) -> Result<(), Refusal> {
    for name in ["minimum-selected-count", "maximum-selected-count"] {
        if optional_property(face, collection, name)?.is_some() {
            return Err(Refusal::UnsupportedProfile);
        }
    }
    let inventory = match optional_property(face, collection, "inventory-count")? {
        None => items.len() as u64,
        Some(FacePropertyValue::Count(n)) => *n,
        _ => return Err(Refusal::Type),
    };
    let visible = items.len() as u64;
    if inventory < visible {
        return Err(Refusal::Membership);
    }
    if let Some(declared) = optional_property(face, collection, "selected-count")? {
        let FacePropertyValue::Count(total) = declared else {
            return Err(Refusal::Type);
        };
        let mut chosen = 0;
        for item in items {
            chosen += u64::from(selected(face, &item.identity)?);
        }
        // Inventory already covers every visible item; subtract first so even
        // a full u64 inventory preserves the exact bound without overflow.
        let maximum_selected = inventory - visible + chosen;
        if *total < chosen || *total > maximum_selected {
            return Err(Refusal::Selection);
        }
    }
    Ok(())
}

fn independent<'a>(face: &'a Face, item: &str) -> Result<&'a FaceAction, Refusal> {
    let id = identity(face, item, "boolean-change-action")?;
    let (index, action) = face
        .actions
        .iter()
        .enumerate()
        .find(|(_, a)| a.identity == id && a.target == item)
        .ok_or(Refusal::Action)?;
    reversible_boolean_value(face, index).ok_or(Refusal::Action)?;
    Ok(action)
}

/// Availability is separate. Both Boolean replacements and one exact Source
/// association are required; a one-way action never becomes a toggle.
fn reversible_boolean_value(face: &Face, action_index: usize) -> Option<bool> {
    let action = face.actions.get(action_index)?;
    if action.arguments.len() != 1 {
        return None;
    }
    let argument = &action.arguments[0];
    if argument.contract.value_kind.as_str() != "value/bool"
        || argument.contract.maximum_bytes != 1
        || argument.contract.validate(&[0]).is_err()
        || argument.contract.validate(&[1]).is_err()
    {
        return None;
    }
    let mut values = face
        .properties
        .iter()
        .filter(|property| property.subject == action.target && property.name == "selected");
    let FacePropertyValue::Flag(value) = &values.next()?.value else {
        return None;
    };
    if values.next().is_some() {
        return None;
    }
    let mut bindings = face.properties.iter().filter(|property| {
        property.subject == action.target && property.name == "boolean-change-action"
    });
    let FacePropertyValue::Identity(identity) = &bindings.next()?.value else {
        return None;
    };
    if identity != &action.identity || bindings.next().is_some() {
        return None;
    }
    Some(*value)
}
