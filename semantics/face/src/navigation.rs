//! Finite, renderer-independent navigation over one exact Face.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    identity::{hash_relationship, hash_string},
    Face, FaceAspect, FaceContentId, FaceDepth, FacePlace, FaceRelationshipKind, NavigationRefusal,
    MAX_FACE_ID_BYTES, MAX_FACE_TEXT_BYTES,
};

pub const MAX_NAVIGATION_PLACES: usize = 3;
pub const MAX_NAVIGATION_ASPECTS_PER_PLACE: usize = 4;
pub const MAX_NAVIGATION_SUBJECTS_PER_ASPECT: usize = 1_024;
pub const MAX_NAVIGATION_FOLLOWS: usize = 2_048;
pub const MAX_NAVIGATION_HISTORY: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationContentId(String);

impl NavigationContentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The portable navigation position over one exact Face revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceCursor {
    pub face: FaceContentId,
    pub navigation: NavigationContentId,
    pub revision: u64,
    pub place: FacePlace,
    pub aspect: FaceAspect,
    pub focus: Option<String>,
    pub depth: FaceDepth,
}

/// One explicitly available Aspect and the subjects admitted for Focus there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationAspect {
    pub aspect: FaceAspect,
    pub focusable_subjects: Vec<String>,
}

/// One explicitly available Place with its exact current root subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationPlace {
    pub place: FacePlace,
    pub root_subject: String,
    pub name: String,
    pub aspects: Vec<NavigationAspect>,
}

/// One exact advertised Face relationship and its navigation destination.
///
/// Navigation may traverse either direction of the relationship. `source_subject`
/// and `target_subject` describe the FOLLOW direction, while `relationship`
/// preserves the exact underlying semantic relationship kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationFollow {
    pub identity: String,
    pub source_subject: String,
    pub relationship: FaceRelationshipKind,
    pub target_subject: String,
    pub target_place: FacePlace,
    pub target_aspect: FaceAspect,
}

/// Finite navigation choices derived from one immutable Face revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceNavigation {
    pub identity: NavigationContentId,
    pub face: FaceContentId,
    pub revision: u64,
    pub places: Vec<NavigationPlace>,
    pub follows: Vec<NavigationFollow>,
}

/// Pure cursor operations. Semantic action invocation is deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigationOperation {
    Enter(FacePlace),
    Show(FaceAspect),
    Focus(String),
    FocusAndDisclose(String, FaceDepth),
    Follow(String),
    Disclose(FaceDepth),
    Back,
}

/// Bounded cursor state. It contains no semantic action or runtime mutation hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationState {
    cursor: FaceCursor,
    history: Vec<FaceCursor>,
    history_limit: usize,
}

impl FaceNavigation {
    pub fn new(
        face: &Face,
        places: Vec<NavigationPlace>,
        follows: Vec<NavigationFollow>,
    ) -> Result<Self, NavigationRefusal> {
        let mut navigation = Self {
            identity: NavigationContentId(String::new()),
            face: face.identity.clone(),
            revision: face.revision,
            places,
            follows,
        };
        navigation.identity = NavigationContentId(navigation.content_digest());
        navigation.validate(face)?;
        Ok(navigation)
    }

    pub fn validate(&self, face: &Face) -> Result<(), NavigationRefusal> {
        if self.face != face.identity || self.revision != face.revision {
            return Err(NavigationRefusal::StaleFace);
        }
        if self.identity.0 != self.content_digest() {
            return Err(NavigationRefusal::InvalidTruth);
        }
        if self.places.is_empty() || self.places.len() > MAX_NAVIGATION_PLACES {
            return Err(NavigationRefusal::InvalidTruth);
        }
        if self.follows.len() > MAX_NAVIGATION_FOLLOWS {
            return Err(NavigationRefusal::InvalidTruth);
        }
        for (index, place) in self.places.iter().enumerate() {
            if self.places[index + 1..]
                .iter()
                .any(|candidate| candidate.place == place.place)
                || !valid_id(&place.root_subject)
                || !valid_text(&place.name)
                || !face.has_subject(&place.root_subject)
                || place.aspects.is_empty()
                || place.aspects.len() > MAX_NAVIGATION_ASPECTS_PER_PLACE
            {
                return Err(NavigationRefusal::InvalidTruth);
            }
            for (aspect_index, aspect) in place.aspects.iter().enumerate() {
                if place.aspects[aspect_index + 1..]
                    .iter()
                    .any(|candidate| candidate.aspect == aspect.aspect)
                    || aspect.focusable_subjects.len() > MAX_NAVIGATION_SUBJECTS_PER_ASPECT
                    || aspect.focusable_subjects.iter().any(|subject| {
                        !valid_id(subject)
                            || !face.has_subject(subject)
                            || aspect
                                .focusable_subjects
                                .iter()
                                .filter(|candidate| *candidate == subject)
                                .count()
                                != 1
                    })
                {
                    return Err(NavigationRefusal::InvalidTruth);
                }
            }
        }
        for (index, follow) in self.follows.iter().enumerate() {
            if !valid_id(&follow.identity)
                || self.follows[index + 1..]
                    .iter()
                    .any(|candidate| candidate.identity == follow.identity)
                || !face.relationships.iter().any(|relationship| {
                    relationship.kind == follow.relationship
                        && ((relationship.source == follow.source_subject
                            && relationship.target == follow.target_subject)
                            || (relationship.source == follow.target_subject
                                && relationship.target == follow.source_subject))
                })
                || !self.places.iter().any(|place| {
                    place.aspects.iter().any(|aspect| {
                        aspect
                            .focusable_subjects
                            .iter()
                            .any(|subject| subject == &follow.source_subject)
                    })
                })
                || !self.subject_is_focusable(
                    follow.target_place,
                    follow.target_aspect,
                    &follow.target_subject,
                )
            {
                return Err(NavigationRefusal::InvalidTruth);
            }
        }
        Ok(())
    }

    fn content_digest(&self) -> String {
        let mut digest = Sha256::new();
        hash_string(&mut digest, "conduit.face/navigation@1");
        hash_string(&mut digest, self.face.as_str());
        digest.update(self.revision.to_le_bytes());
        for place in &self.places {
            digest.update([place.place as u8]);
            hash_string(&mut digest, &place.root_subject);
            hash_string(&mut digest, &place.name);
            for aspect in &place.aspects {
                digest.update([aspect.aspect as u8]);
                for subject in &aspect.focusable_subjects {
                    hash_string(&mut digest, subject);
                }
            }
        }
        for follow in &self.follows {
            hash_string(&mut digest, &follow.identity);
            hash_string(&mut digest, &follow.source_subject);
            hash_relationship(&mut digest, &follow.relationship);
            hash_string(&mut digest, &follow.target_subject);
            digest.update([follow.target_place as u8, follow.target_aspect as u8]);
        }
        let bytes: [u8; 32] = digest.finalize().into();
        hex(&bytes)
    }

    fn place(&self, place: FacePlace) -> Option<&NavigationPlace> {
        self.places
            .iter()
            .find(|candidate| candidate.place == place)
    }

    fn aspect(&self, place: FacePlace, aspect: FaceAspect) -> Option<&NavigationAspect> {
        self.place(place)?
            .aspects
            .iter()
            .find(|candidate| candidate.aspect == aspect)
    }

    fn subject_is_focusable(&self, place: FacePlace, aspect: FaceAspect, subject: &str) -> bool {
        self.aspect(place, aspect).is_some_and(|available| {
            available
                .focusable_subjects
                .iter()
                .any(|candidate| candidate == subject)
        })
    }

    fn validate_cursor(&self, cursor: &FaceCursor) -> Result<(), NavigationRefusal> {
        if cursor.face != self.face
            || cursor.navigation != self.identity
            || cursor.revision != self.revision
        {
            return Err(NavigationRefusal::StaleFace);
        }
        let Some(aspect) = self.aspect(cursor.place, cursor.aspect) else {
            return Err(if self.place(cursor.place).is_some() {
                NavigationRefusal::UnknownAspect
            } else {
                NavigationRefusal::UnknownPlace
            });
        };
        if cursor.focus.as_ref().is_some_and(|focus| {
            !aspect
                .focusable_subjects
                .iter()
                .any(|subject| subject == focus)
        }) {
            return Err(NavigationRefusal::UnknownSubject);
        }
        Ok(())
    }
}

impl NavigationState {
    pub fn new(
        navigation: &FaceNavigation,
        cursor: FaceCursor,
        history_limit: usize,
    ) -> Result<Self, NavigationRefusal> {
        if history_limit > MAX_NAVIGATION_HISTORY {
            return Err(NavigationRefusal::InvalidTruth);
        }
        navigation.validate_cursor(&cursor)?;
        Ok(Self {
            cursor,
            history: Vec::with_capacity(history_limit),
            history_limit,
        })
    }

    pub fn cursor(&self) -> &FaceCursor {
        &self.cursor
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn navigate(
        &mut self,
        face: &Face,
        navigation: &FaceNavigation,
        revision: u64,
        operation: NavigationOperation,
    ) -> Result<&FaceCursor, NavigationRefusal> {
        navigation.validate(face)?;
        navigation.validate_cursor(&self.cursor)?;
        if revision != navigation.revision {
            return Err(NavigationRefusal::StaleFace);
        }
        if operation == NavigationOperation::Back {
            self.cursor = self
                .history
                .pop()
                .ok_or(NavigationRefusal::HistoryExhausted)?;
            return Ok(&self.cursor);
        }
        let mut next = self.cursor.clone();
        match operation {
            NavigationOperation::Enter(place) => {
                let available = navigation
                    .place(place)
                    .ok_or(NavigationRefusal::UnknownPlace)?;
                next.place = place;
                next.aspect = available.aspects[0].aspect;
                next.focus = None;
                next.depth = FaceDepth::Primary;
            }
            NavigationOperation::Show(aspect) => {
                navigation
                    .aspect(next.place, aspect)
                    .ok_or(NavigationRefusal::UnknownAspect)?;
                next.aspect = aspect;
                next.focus = None;
            }
            NavigationOperation::Focus(subject) => {
                if !navigation.subject_is_focusable(next.place, next.aspect, &subject) {
                    return Err(NavigationRefusal::UnknownSubject);
                }
                next.focus = Some(subject);
            }
            NavigationOperation::FocusAndDisclose(subject, depth) => {
                if !navigation.subject_is_focusable(next.place, next.aspect, &subject) {
                    return Err(NavigationRefusal::UnknownSubject);
                }
                next.focus = Some(subject);
                next.depth = depth;
            }
            NavigationOperation::Follow(identity) => {
                let source = next
                    .focus
                    .as_deref()
                    .ok_or(NavigationRefusal::UnknownSubject)?;
                let follow = navigation
                    .follows
                    .iter()
                    .find(|candidate| {
                        candidate.identity == identity && candidate.source_subject == source
                    })
                    .ok_or(NavigationRefusal::UnknownRelationship)?;
                next.place = follow.target_place;
                next.aspect = follow.target_aspect;
                next.focus = Some(follow.target_subject.clone());
            }
            NavigationOperation::Disclose(depth) => next.depth = depth,
            NavigationOperation::Back => unreachable!(),
        }
        navigation.validate_cursor(&next)?;
        if self.history.len() == self.history_limit {
            return Err(NavigationRefusal::HistoryFull);
        }
        self.history.push(self.cursor.clone());
        self.cursor = next;
        Ok(&self.cursor)
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_FACE_ID_BYTES
}

fn valid_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_FACE_TEXT_BYTES
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
