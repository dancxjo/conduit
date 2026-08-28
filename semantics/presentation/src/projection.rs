//! Finite Place x Aspect projection over exact Presentation content.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    identity::hash_string, NavigationContentId, Presentation, PresentationAspect,
    PresentationContentId, PresentationCursor, PresentationDepth, PresentationNavigation,
    PresentationPlace,
};

pub const MAX_PROJECTION_MEMBERSHIPS: usize = 16_384;
pub const MAX_PROJECTION_MEMBERSHIPS_PER_ITEM: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectionContentId(String);

impl ProjectionContentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An exact reference into one immutable Presentation.
///
/// Ordinals are safe here because the containing projection is bound to the
/// Presentation content identity. They do not create place-specific copies of
/// semantic subjects or content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionItem {
    Subject(String),
    Relationship(u16),
    Property(u16),
    Text(u16),
    Action(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionMembership {
    pub place: PresentationPlace,
    pub aspect: PresentationAspect,
    pub item: ProjectionItem,
    pub depth: PresentationDepth,
}

/// Projection membership truth for one exact Presentation and navigation map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationProjection {
    pub identity: ProjectionContentId,
    pub presentation: PresentationContentId,
    pub navigation: NavigationContentId,
    pub revision: u64,
    pub memberships: Vec<ProjectionMembership>,
}

/// A deterministic view of the items admitted by one current cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedPresentation<'a> {
    pub projection: &'a ProjectionContentId,
    pub presentation: PresentationContentId,
    pub cursor: PresentationCursor,
    pub items: Vec<&'a ProjectionMembership>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionRefusal {
    StalePresentation,
    StaleNavigation,
    UnknownPlace,
    UnknownAspect,
    UnknownItem,
    DuplicateMembership,
    TooManyMemberships,
    TooManyMembershipsForItem,
    MissingFollowRelationship,
    InvalidTruth,
}

impl PresentationProjection {
    pub fn new(
        presentation: &Presentation,
        navigation: &PresentationNavigation,
        memberships: Vec<ProjectionMembership>,
    ) -> Result<Self, ProjectionRefusal> {
        navigation
            .validate(presentation)
            .map_err(|_| ProjectionRefusal::StaleNavigation)?;
        let mut projection = Self {
            identity: ProjectionContentId(String::new()),
            presentation: presentation.identity.clone(),
            navigation: navigation.identity.clone(),
            revision: presentation.revision,
            memberships,
        };
        projection.identity = ProjectionContentId(projection.content_digest());
        projection.validate(presentation, navigation)?;
        Ok(projection)
    }

    pub fn validate(
        &self,
        presentation: &Presentation,
        navigation: &PresentationNavigation,
    ) -> Result<(), ProjectionRefusal> {
        if self.presentation != presentation.identity || self.revision != presentation.revision {
            return Err(ProjectionRefusal::StalePresentation);
        }
        if self.navigation != navigation.identity {
            return Err(ProjectionRefusal::StaleNavigation);
        }
        navigation
            .validate(presentation)
            .map_err(|_| ProjectionRefusal::StaleNavigation)?;
        if self.identity.0 != self.content_digest() {
            return Err(ProjectionRefusal::InvalidTruth);
        }
        if self.memberships.len() > MAX_PROJECTION_MEMBERSHIPS {
            return Err(ProjectionRefusal::TooManyMemberships);
        }
        for (index, membership) in self.memberships.iter().enumerate() {
            let Some(place) = navigation
                .places
                .iter()
                .find(|place| place.place == membership.place)
            else {
                return Err(ProjectionRefusal::UnknownPlace);
            };
            if !place
                .aspects
                .iter()
                .any(|aspect| aspect.aspect == membership.aspect)
            {
                return Err(ProjectionRefusal::UnknownAspect);
            }
            if !item_exists(presentation, &membership.item) {
                return Err(ProjectionRefusal::UnknownItem);
            }
            if self.memberships[index + 1..].iter().any(|candidate| {
                candidate.place == membership.place
                    && candidate.aspect == membership.aspect
                    && candidate.item == membership.item
            }) {
                return Err(ProjectionRefusal::DuplicateMembership);
            }
            if self
                .memberships
                .iter()
                .filter(|candidate| candidate.item == membership.item)
                .count()
                > MAX_PROJECTION_MEMBERSHIPS_PER_ITEM
            {
                return Err(ProjectionRefusal::TooManyMembershipsForItem);
            }
        }
        for follow in &navigation.follows {
            let relationship = presentation.relationships.iter().position(|candidate| {
                candidate.kind == follow.relationship
                    && ((candidate.source == follow.source_subject
                        && candidate.target == follow.target_subject)
                        || (candidate.source == follow.target_subject
                            && candidate.target == follow.source_subject))
            });
            if relationship.is_none_or(|relationship| {
                !source_places(navigation, &follow.source_subject).any(|source_place| {
                    self.memberships.iter().any(|membership| {
                        membership.place == source_place
                            && membership.item == ProjectionItem::Relationship(relationship as u16)
                    })
                })
            }) {
                return Err(ProjectionRefusal::MissingFollowRelationship);
            }
        }
        Ok(())
    }

    pub fn project<'a>(
        &'a self,
        presentation: &Presentation,
        navigation: &PresentationNavigation,
        cursor: &PresentationCursor,
    ) -> Result<ProjectedPresentation<'a>, ProjectionRefusal> {
        self.validate(presentation, navigation)?;
        if cursor.presentation != self.presentation || cursor.revision != self.revision {
            return Err(ProjectionRefusal::StalePresentation);
        }
        if cursor.navigation != self.navigation {
            return Err(ProjectionRefusal::StaleNavigation);
        }
        let Some(place) = navigation
            .places
            .iter()
            .find(|place| place.place == cursor.place)
        else {
            return Err(ProjectionRefusal::UnknownPlace);
        };
        let Some(aspect) = place
            .aspects
            .iter()
            .find(|aspect| aspect.aspect == cursor.aspect)
        else {
            return Err(ProjectionRefusal::UnknownAspect);
        };
        if cursor.focus.as_ref().is_some_and(|focus| {
            !aspect
                .focusable_subjects
                .iter()
                .any(|subject| subject == focus)
        }) {
            return Err(ProjectionRefusal::UnknownItem);
        }
        Ok(ProjectedPresentation {
            projection: &self.identity,
            presentation: self.presentation.clone(),
            cursor: cursor.clone(),
            items: self
                .memberships
                .iter()
                .filter(|membership| {
                    membership.place == cursor.place
                        && membership.aspect == cursor.aspect
                        && membership.depth <= cursor.depth
                })
                .collect(),
        })
    }

    fn content_digest(&self) -> String {
        let mut digest = Sha256::new();
        hash_string(&mut digest, "conduit.presentation/projection@1");
        hash_string(&mut digest, self.presentation.as_str());
        hash_string(&mut digest, self.navigation.as_str());
        digest.update(self.revision.to_le_bytes());
        for membership in &self.memberships {
            digest.update([
                membership.place as u8,
                membership.aspect as u8,
                membership.depth as u8,
            ]);
            hash_item(&mut digest, &membership.item);
        }
        hex(&digest.finalize())
    }
}

fn item_exists(presentation: &Presentation, item: &ProjectionItem) -> bool {
    match item {
        ProjectionItem::Subject(identity) => presentation.has_subject(identity),
        ProjectionItem::Relationship(index) => {
            usize::from(*index) < presentation.relationships.len()
        }
        ProjectionItem::Property(index) => usize::from(*index) < presentation.properties.len(),
        ProjectionItem::Text(index) => usize::from(*index) < presentation.text.len(),
        ProjectionItem::Action(identity) => presentation
            .actions
            .iter()
            .any(|action| action.identity == *identity),
    }
}

fn source_places<'a>(
    navigation: &'a PresentationNavigation,
    subject: &'a str,
) -> impl Iterator<Item = PresentationPlace> + 'a {
    navigation
        .places
        .iter()
        .filter(move |place| {
            place.aspects.iter().any(|aspect| {
                aspect
                    .focusable_subjects
                    .iter()
                    .any(|candidate| candidate == subject)
            })
        })
        .map(|place| place.place)
}

fn hash_item(digest: &mut Sha256, item: &ProjectionItem) {
    match item {
        ProjectionItem::Subject(identity) => {
            digest.update([0]);
            hash_string(digest, identity);
        }
        ProjectionItem::Relationship(index) => hash_ordinal(digest, 1, *index),
        ProjectionItem::Property(index) => hash_ordinal(digest, 2, *index),
        ProjectionItem::Text(index) => hash_ordinal(digest, 3, *index),
        ProjectionItem::Action(identity) => {
            digest.update([4]);
            hash_string(digest, identity);
        }
    }
}

fn hash_ordinal(digest: &mut Sha256, tag: u8, index: u16) {
    digest.update([tag]);
    digest.update(index.to_le_bytes());
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
