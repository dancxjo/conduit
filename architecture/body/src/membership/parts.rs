//! Shared immutable member truth; mutation obtains an independent Vec first.
use super::PartMembership;
use alloc::vec::Vec;
use core::{
    fmt,
    ops::{Deref, DerefMut},
};
use portable_atomic_util::Arc;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Ordered current/checkpoint membership with copy-on-write owned mutation.
/// Values and wire shape remain an ordinary ordered member array. Clone shares
/// immutable storage; any mutable Vec access first separates shared storage.
#[derive(Clone, Default)]
pub struct MembershipParts(Option<Arc<Vec<PartMembership>>>);
impl PartialEq for MembershipParts {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for MembershipParts {}
impl From<Vec<PartMembership>> for MembershipParts {
    fn from(parts: Vec<PartMembership>) -> Self {
        if parts.capacity() == 0 {
            Self(None)
        } else {
            Self(Some(Arc::new(parts)))
        }
    }
}
impl Deref for MembershipParts {
    type Target = Vec<PartMembership>;
    fn deref(&self) -> &Self::Target {
        static EMPTY: Vec<PartMembership> = Vec::new();
        self.0.as_deref().unwrap_or(&EMPTY)
    }
}
impl DerefMut for MembershipParts {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(Vec::new())))
    }
}
impl fmt::Debug for MembershipParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.deref(), f)
    }
}
impl Serialize for MembershipParts {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.deref().serialize(s)
    }
}
impl<'de> Deserialize<'de> for MembershipParts {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Vec::deserialize(d).map(Self::from)
    }
}
impl<'a> IntoIterator for &'a MembershipParts {
    type Item = &'a PartMembership;
    type IntoIter = core::slice::Iter<'a, PartMembership>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a> IntoIterator for &'a mut MembershipParts {
    type Item = &'a mut PartMembership;
    type IntoIter = core::slice::IterMut<'a, PartMembership>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl IntoIterator for MembershipParts {
    type Item = PartMembership;
    type IntoIter = alloc::vec::IntoIter<PartMembership>;
    fn into_iter(self) -> Self::IntoIter {
        self.0
            .map_or_else(Vec::new, |parts| {
                Arc::try_unwrap(parts).unwrap_or_else(|shared| (*shared).clone())
            })
            .into_iter()
    }
}

impl core::iter::FromIterator<PartMembership> for MembershipParts {
    fn from_iter<I: IntoIterator<Item = PartMembership>>(iter: I) -> Self {
        Self::from(iter.into_iter().collect::<Vec<_>>())
    }
}
impl From<MembershipParts> for Vec<PartMembership> {
    fn from(parts: MembershipParts) -> Self {
        parts.into_iter().collect()
    }
}
#[cfg(test)]
impl MembershipParts {
    pub(super) fn shares_storage_with(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Body, MembershipState, PartId};
    fn part() -> PartMembership {
        let body = Body::born(
            "source/cow".into(),
            "checked/cow".into(),
            1,
            "sign/cow".into(),
        )
        .unwrap();
        PartMembership {
            part_id: PartId::bind(&body.body_id, "subject/cow", 1).unwrap(),
            state: MembershipState::Admitted,
            current: None,
        }
    }
    #[test]
    fn clone_is_shared_but_mutation_keeps_prior_truth_and_order() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<MembershipParts>();
        let original = MembershipParts::from(alloc::vec![part()]);
        let mut changed = original.clone();
        assert!(Arc::ptr_eq(
            original.0.as_ref().unwrap(),
            changed.0.as_ref().unwrap()
        ));
        changed[0].state = MembershipState::Revoked;
        assert_eq!(original[0].state, MembershipState::Admitted);
        assert_eq!(changed[0].state, MembershipState::Revoked);
        assert!(!Arc::ptr_eq(
            original.0.as_ref().unwrap(),
            changed.0.as_ref().unwrap()
        ));
        assert_eq!((&original).into_iter().count(), 1);
        assert_eq!((&mut changed).into_iter().count(), 1);
    }
    #[test]
    fn wire_is_exact_vec_array_and_empty_storage_has_value_equality() {
        let rows = alloc::vec![part()];
        let original = MembershipParts::from(rows.clone());
        assert_eq!(
            serde_json::to_vec(&original).unwrap(),
            serde_json::to_vec(&rows).unwrap()
        );
        assert_eq!(
            postcard::to_allocvec(&original).unwrap(),
            postcard::to_allocvec(&rows).unwrap()
        );
        let decoded: MembershipParts =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        assert_eq!(decoded, original);
        let binary = postcard::to_allocvec(&rows).unwrap();
        let decoded: MembershipParts = postcard::from_bytes(&binary).unwrap();
        assert_eq!(decoded, original);
        let mut cleared = original;
        cleared.clear();
        assert_eq!(cleared, MembershipParts::default());
        assert_eq!(serde_json::to_vec(&cleared).unwrap(), b"[]");
    }
}
