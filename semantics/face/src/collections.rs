//! Borrow bounded collection meaning from one immutable source Face.
//! Pages and focus are Mask-local views; only explicit interaction construction
//! can propose a source action, through the existing Show-bound contract.
use crate::{
    ChoiceMultiplicity, Face, FaceAction, FaceInteraction, FaceInteractionArgument,
    FaceInteractionRefusal, FacePropertyValue, FaceRelationshipKind, FaceRole, FaceSubject,
    MaskShow,
};
use alloc::{collections::BTreeSet, vec, vec::Vec};

pub const MAX_PREPARED_COLLECTION_ITEMS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceCollectionRefusal {
    InvalidFace,
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
    UnknownItem,
    NoSelectionAction,
    StaleFace,
    Interaction(FaceInteractionRefusal),
}
use FaceCollectionRefusal as Refusal;

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

#[derive(Debug)]
pub struct PreparedFaceCollection<'a> {
    face: &'a Face,
    subject: &'a FaceSubject,
    items: Vec<&'a FaceSubject>,
    multiplicity: Option<ChoiceMultiplicity>,
    exclusive_action: Option<&'a FaceAction>,
}

pub struct FaceCollectionPage<'view, 'face> {
    pub face_identity: &'face str,
    pub revision: u64,
    pub collection_identity: &'face str,
    pub items: &'view [&'face FaceSubject],
    pub total: usize,
    pub next: Option<usize>,
}

pub fn prepare_face_collection<'a>(
    face: &'a Face,
    collection: &str,
    maximum_items: usize,
) -> Result<PreparedFaceCollection<'a>, Refusal> {
    face.validate().map_err(|_| Refusal::InvalidFace)?;
    prepare_validated_collection(face, collection, maximum_items)
}

pub(crate) fn prepare_validated_collection<'a>(
    face: &'a Face,
    collection: &str,
    maximum_items: usize,
) -> Result<PreparedFaceCollection<'a>, Refusal> {
    if maximum_items == 0 || maximum_items > MAX_PREPARED_COLLECTION_ITEMS {
        return Err(Refusal::Capacity);
    }
    let subject = face
        .subjects
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
    let exclusive_action = match multiplicity {
        Some(ChoiceMultiplicity::Exclusive) => Some(exclusive(face, collection, &items)?),
        Some(ChoiceMultiplicity::Independent) => {
            independent_counts(face, collection, &items)?;
            for item in &items {
                independent(face, &item.identity)?;
            }
            None
        }
        None => None,
    };
    Ok(PreparedFaceCollection {
        face,
        subject,
        items,
        multiplicity,
        exclusive_action,
    })
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
    crate::reversible_boolean_value(face, index).ok_or(Refusal::Action)?;
    Ok(action)
}

impl<'a> PreparedFaceCollection<'a> {
    pub fn source(&self) -> &'a Face {
        self.face
    }
    pub fn subject(&self) -> &'a FaceSubject {
        self.subject
    }
    pub fn items(&self) -> &[&'a FaceSubject] {
        &self.items
    }
    pub fn multiplicity(&self) -> Option<ChoiceMultiplicity> {
        self.multiplicity
    }
    pub fn page(
        &self,
        first: usize,
        maximum: usize,
    ) -> Result<FaceCollectionPage<'_, 'a>, Refusal> {
        if first > self.items.len() || maximum == 0 || maximum > MAX_PREPARED_COLLECTION_ITEMS {
            return Err(Refusal::Capacity);
        }
        let end = first.saturating_add(maximum).min(self.items.len());
        Ok(FaceCollectionPage {
            face_identity: self.face.identity.as_str(),
            revision: self.face.revision,
            collection_identity: &self.subject.identity,
            items: &self.items[first..end],
            total: self.items.len(),
            next: (end < self.items.len()).then_some(end),
        })
    }
    /// Construct a proposal only. The source owner still validates and commits
    /// through ordinary execution; paging, focus and highlight never call this.
    pub fn interaction(
        &self,
        current: &Face,
        show: &MaskShow,
        item: &str,
        choose: bool,
        sequence: u64,
    ) -> Result<FaceInteraction, Refusal> {
        current.validate().map_err(|_| Refusal::InvalidFace)?;
        if current.identity != self.face.identity || current.revision != self.face.revision {
            return Err(Refusal::StaleFace);
        }
        let item = self
            .items
            .iter()
            .find(|s| s.identity == item)
            .ok_or(Refusal::UnknownItem)?;
        let (action, bytes) = match self.multiplicity {
            None => return Err(Refusal::NoSelectionAction),
            Some(ChoiceMultiplicity::Exclusive) => {
                if !choose {
                    return Err(Refusal::Selection);
                }
                (
                    self.exclusive_action.expect("admitted exclusive action"),
                    value(self.face, &item.identity, "choice-value")?.1.to_vec(),
                )
            }
            Some(ChoiceMultiplicity::Independent) => (
                independent(self.face, &item.identity)?,
                vec![u8::from(choose)],
            ),
        };
        let argument = &action.arguments[0];
        FaceInteraction::new(
            current,
            show,
            &action.identity,
            &action.target,
            vec![FaceInteractionArgument {
                name: argument.name.clone(),
                value_kind: argument.contract.value_kind.as_str().into(),
                value: bytes,
            }],
            sequence,
        )
        .map_err(Refusal::Interaction)
    }
}
