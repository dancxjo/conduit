//! Immutable bounded collection views and proposals through the ordinary Show contract.
//! Preparation and paging never mutate the source or execute an action.
use crate::{
    collection_facts::{self, PreparedCollectionFacts},
    ChoiceMultiplicity, CollectionFactsRefusal, Face, FaceInteraction, FaceInteractionArgument,
    FaceInteractionRefusal, FaceSubject, MaskShow,
};
use alloc::vec;
use FaceCollectionRefusal::*;

pub const MAX_PREPARED_COLLECTION_ITEMS: usize = collection_facts::MAX_PREPARED_COLLECTION_ITEMS;

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

impl From<CollectionFactsRefusal> for FaceCollectionRefusal {
    fn from(value: CollectionFactsRefusal) -> Self {
        match value {
            CollectionFactsRefusal::MissingCollection => MissingCollection,
            CollectionFactsRefusal::MissingFact => MissingFact,
            CollectionFactsRefusal::AmbiguousFact => AmbiguousFact,
            CollectionFactsRefusal::Type => Type,
            CollectionFactsRefusal::Capacity => Capacity,
            CollectionFactsRefusal::Membership => Membership,
            CollectionFactsRefusal::Order => Order,
            CollectionFactsRefusal::Selection => Selection,
            CollectionFactsRefusal::Action => Action,
            CollectionFactsRefusal::UnsupportedProfile => UnsupportedProfile,
        }
    }
}

#[derive(Debug)]
pub struct PreparedFaceCollection<'a> {
    face: &'a Face,
    facts: PreparedCollectionFacts<'a>,
}

#[derive(Debug)]
pub struct FaceCollectionPage<'view, 'face> {
    pub face_identity: &'face str,
    pub revision: u64,
    pub collection_identity: &'face str,
    pub items: &'view [&'face FaceSubject],
    pub total: usize,
    pub next: Option<usize>,
}

/// Validate the complete source before borrowing its exact collection facts.
/// The bound admits preparation only, not renderer storage or execution.
pub fn prepare_face_collection<'a>(
    face: &'a Face,
    collection: &str,
    maximum_items: usize,
) -> Result<PreparedFaceCollection<'a>, FaceCollectionRefusal> {
    face.validate().map_err(|_| InvalidFace)?;
    let facts = collection_facts::prepare_collection(face, collection, maximum_items)?;
    Ok(PreparedFaceCollection { face, facts })
}

impl<'a> PreparedFaceCollection<'a> {
    pub fn source(&self) -> &'a Face {
        self.face
    }
    pub fn subject(&self) -> &'a FaceSubject {
        self.facts.subject
    }
    pub fn items(&self) -> &[&'a FaceSubject] {
        &self.facts.items
    }
    pub fn multiplicity(&self) -> Option<ChoiceMultiplicity> {
        self.facts.multiplicity
    }
    /// View source order without changing selection, focus or membership.
    pub fn page(
        &self,
        first: usize,
        maximum: usize,
    ) -> Result<FaceCollectionPage<'_, 'a>, FaceCollectionRefusal> {
        if first > self.facts.items.len() || maximum == 0 || maximum > MAX_PREPARED_COLLECTION_ITEMS
        {
            return Err(Capacity);
        }
        let end = first.saturating_add(maximum).min(self.facts.items.len());
        Ok(FaceCollectionPage {
            face_identity: self.face.identity.as_str(),
            revision: self.face.revision,
            collection_identity: &self.facts.subject.identity,
            items: &self.facts.items[first..end],
            total: self.facts.items.len(),
            next: (end < self.facts.items.len()).then_some(end),
        })
    }
    /// Construct a proposal only. The existing Show/action contract validates
    /// availability and exact arguments; the source owner still owns commitment.
    pub fn interaction(
        &self,
        current: &Face,
        show: &MaskShow,
        item: &str,
        choose: bool,
        sequence: u64,
    ) -> Result<FaceInteraction, FaceCollectionRefusal> {
        current.validate().map_err(|_| InvalidFace)?;
        if current.identity != self.face.identity || current.revision != self.face.revision {
            return Err(StaleFace);
        }
        let item = self
            .facts
            .items
            .iter()
            .find(|s| s.identity == item)
            .ok_or(UnknownItem)?;
        let (action, bytes) = match self.facts.multiplicity {
            None => return Err(NoSelectionAction),
            Some(ChoiceMultiplicity::Exclusive) => {
                if !choose {
                    return Err(Selection);
                }
                (
                    self.facts
                        .exclusive_action
                        .expect("validated exclusive action"),
                    collection_facts::value(self.face, &item.identity, "choice-value")?
                        .1
                        .to_vec(),
                )
            }
            Some(ChoiceMultiplicity::Independent) => (
                collection_facts::independent(self.face, &item.identity)?,
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
        .map_err(Interaction)
    }
}
