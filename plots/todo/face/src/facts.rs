//! Opaque references to Todo-owned facts in one immutable Face generation.
//! Counts come from source properties, never prose or the displayed Item page.
use alloc::vec::Vec;
use conduit_face::{
    Face, FaceAction, FaceActionAvailability, FaceDisclosureLevel, FaceProperty, FacePropertyValue,
    FaceRelationshipKind, FaceRole, FaceSubject,
};
use conduit_todo_plot::{MAX_TODO_ITEMS, MAX_TODO_TEXT_BYTES, MAX_TODO_TITLE_BYTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoFactRefusal {
    Face,
    Missing,
    Duplicate,
    Type,
    Bound,
    Disclosure,
    Stale,
    Item,
    Action,
}

pub struct TodoFaceFacts<'a> {
    face: &'a Face,
    list: &'a FaceSubject,
    remaining: &'a FaceProperty,
    completed: &'a FaceProperty,
}
fn property<'a>(
    face: &'a Face,
    subject: &str,
    name: &str,
) -> Result<&'a FaceProperty, TodoFactRefusal> {
    let mut values = face
        .properties
        .iter()
        .filter(|p| p.subject == subject && p.name == name);
    let value = values.next().ok_or(TodoFactRefusal::Missing)?;
    if values.next().is_some() {
        return Err(TodoFactRefusal::Duplicate);
    }
    Ok(value)
}
fn count(value: &FaceProperty) -> Result<u64, TodoFactRefusal> {
    match value.value {
        FacePropertyValue::Count(value) => Ok(value),
        _ => Err(TodoFactRefusal::Type),
    }
}
fn disclosed(face: &Face, subject: &str) -> Result<(), TodoFactRefusal> {
    let mut levels = face.disclosures.iter().filter(|d| d.subject == subject);
    let level = levels.next().ok_or(TodoFactRefusal::Disclosure)?;
    if levels.next().is_some()
        || !matches!(
            level.level,
            FaceDisclosureLevel::Primary | FaceDisclosureLevel::Context
        )
    {
        return Err(TodoFactRefusal::Disclosure);
    }
    Ok(())
}
impl<'a> TodoFaceFacts<'a> {
    pub fn prepare(face: &'a Face) -> Result<Self, TodoFactRefusal> {
        face.validate().map_err(|_| TodoFactRefusal::Face)?;
        let list = face
            .subjects
            .iter()
            .find(|s| s.identity == "todo/list" && s.role == FaceRole::Collection)
            .ok_or(TodoFactRefusal::Missing)?;
        if list.name.is_empty() || list.name.len() > MAX_TODO_TITLE_BYTES {
            return Err(TodoFactRefusal::Bound);
        }
        disclosed(face, &list.identity)?;
        let remaining = property(face, &list.identity, "remaining-count")?;
        let completed = property(face, &list.identity, "completed-count")?;
        let total = count(remaining)?
            .checked_add(count(completed)?)
            .ok_or(TodoFactRefusal::Bound)?;
        if total > MAX_TODO_ITEMS as u64 {
            return Err(TodoFactRefusal::Bound);
        }
        // The domain revision remains a source fact in the same exact generation.
        count(property(face, &list.identity, "todo-revision")?)?;
        Ok(Self {
            face,
            list,
            remaining,
            completed,
        })
    }
    pub fn source(&self) -> &'a Face {
        self.face
    }
    pub fn list(&self) -> &'a FaceSubject {
        self.list
    }
    pub fn remaining(&self) -> u64 {
        count(self.remaining).expect("privately admitted count")
    }
    pub fn completed(&self) -> u64 {
        count(self.completed).expect("privately admitted count")
    }
    pub fn remaining_property(&self) -> &'a FaceProperty {
        self.remaining
    }
    pub fn completed_property(&self) -> &'a FaceProperty {
        self.completed
    }
    pub fn validate_current(&self, current: &Face) -> Result<(), TodoFactRefusal> {
        current.validate().map_err(|_| TodoFactRefusal::Face)?;
        if current != self.face {
            return Err(TodoFactRefusal::Stale);
        }
        Ok(())
    }
    pub fn remaining_item(&self, identity: &str) -> Result<&'a FaceSubject, TodoFactRefusal> {
        let item = self
            .face
            .subjects
            .iter()
            .find(|s| s.identity == identity && s.role == FaceRole::Item)
            .ok_or(TodoFactRefusal::Item)?;
        if !self.face.relationships.iter().any(|r| {
            r.source == self.list.identity
                && r.target == identity
                && r.kind == FaceRelationshipKind::Contains
        }) {
            return Err(TodoFactRefusal::Item);
        }
        disclosed(self.face, identity)?;
        if item.name.is_empty() || item.name.len() > MAX_TODO_TEXT_BYTES {
            return Err(TodoFactRefusal::Bound);
        }
        if !matches!(
            property(self.face, identity, "complete")?.value,
            FacePropertyValue::Flag(false)
        ) {
            return Err(TodoFactRefusal::Item);
        }
        Ok(item)
    }
    pub fn remaining_page(
        &self,
        identities: &[&str],
    ) -> Result<Vec<&'a FaceSubject>, TodoFactRefusal> {
        if identities.len() > 3 {
            return Err(TodoFactRefusal::Bound);
        }
        let mut values = Vec::with_capacity(identities.len());
        for identity in identities {
            if values
                .iter()
                .any(|s: &&FaceSubject| s.identity == *identity)
            {
                return Err(TodoFactRefusal::Duplicate);
            }
            let item = self.remaining_item(identity)?;
            let order = count(property(self.face, identity, "order")?)?;
            if order >= MAX_TODO_ITEMS as u64 {
                return Err(TodoFactRefusal::Bound);
            }
            if let Some(previous) = values.last() {
                let previous_order = count(property(self.face, &previous.identity, "order")?)?;
                if order <= previous_order {
                    return Err(TodoFactRefusal::Item);
                }
            }
            values.push(item);
        }
        Ok(values)
    }
    pub fn action(
        &self,
        current: &Face,
        identity: &str,
    ) -> Result<&'a FaceAction, TodoFactRefusal> {
        self.validate_current(current)?;
        let action = self
            .face
            .actions
            .iter()
            .find(|a| a.identity == identity)
            .ok_or(TodoFactRefusal::Action)?;
        if action.availability != FaceActionAvailability::Available
            || !action.arguments.is_empty()
            || action.disclosure != FaceDisclosureLevel::CurrentAction
            || action.intent != "todo/complete@1"
        {
            return Err(TodoFactRefusal::Action);
        }
        self.remaining_item(&action.target)?;
        Ok(action)
    }
}
