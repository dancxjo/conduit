//! Bounded borrowed indexes accelerate comparisons, never validate a Face.
//! Indices retain exact equality within a key range, including repeated keys.
use conduit_face::{Face, FaceFragmentError, FaceProperty, FaceRelationship, FaceSubject};

pub(super) struct ContentIndex<'a> {
    pub subjects: Index<'a, FaceSubject, { conduit_face::MAX_FACE_SUBJECTS }>,
    pub relationships: Index<'a, FaceRelationship, { conduit_face::MAX_FACE_RELATIONSHIPS }>,
    pub properties: Index<'a, FaceProperty, { conduit_face::MAX_FACE_PROPERTIES }>,
}
impl<'a> ContentIndex<'a> {
    pub fn new(face: &'a Face) -> Result<Self, FaceFragmentError> {
        Ok(Self {
            subjects: Index::new(&face.subjects, |s| (&s.identity, ""))
                .ok_or(FaceFragmentError::TooManySubjects)?,
            relationships: Index::new(&face.relationships, |r| (&r.source, &r.target))
                .ok_or(FaceFragmentError::TooManyRelationships)?,
            properties: Index::new(&face.properties, |p| (&p.subject, &p.name))
                .ok_or(FaceFragmentError::TooManyProperties)?,
        })
    }
}
/// Fixed u16 offsets into one immutable borrowed slice. No content is cloned.
pub(super) struct Index<'a, T, const N: usize> {
    values: &'a [T],
    order: [u16; N],
    key: fn(&T) -> (&str, &str),
}
impl<'a, T: PartialEq, const N: usize> Index<'a, T, N> {
    fn new(values: &'a [T], key: fn(&T) -> (&str, &str)) -> Option<Self> {
        if values.len() > N || values.len() > usize::from(u16::MAX) {
            return None;
        }
        let mut order = [0; N];
        for (i, item) in order[..values.len()].iter_mut().enumerate() {
            *item = i as u16;
        }
        order[..values.len()].sort_unstable_by(|&a, &b| {
            key(&values[usize::from(a)]).cmp(&key(&values[usize::from(b)]))
        });
        Some(Self { values, order, key })
    }
    pub fn contains(&self, value: &T) -> bool {
        let key = (self.key)(value);
        let order = &self.order[..self.values.len()];
        let first = order.partition_point(|&i| (self.key)(&self.values[usize::from(i)]) < key);
        order[first..]
            .iter()
            .map(|&i| &self.values[usize::from(i)])
            .take_while(|candidate| (self.key)(candidate) == key)
            .any(|candidate| candidate == value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn index_workspace_and_distinct_key_lookup_work_are_bounded() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        static KEYS: AtomicUsize = AtomicUsize::new(0);
        fn key(value: &alloc::string::String) -> (&str, &str) {
            KEYS.fetch_add(1, Ordering::Relaxed);
            (value.as_str(), "")
        }
        let values = (0..1024)
            .rev()
            .map(|i| alloc::format!("{i:04}"))
            .collect::<alloc::vec::Vec<_>>();
        let index = Index::<_, 1024>::new(&values, key).unwrap();
        KEYS.store(0, Ordering::Relaxed);
        for value in &values {
            assert!(index.contains(value));
        }
        assert!(KEYS.load(Ordering::Relaxed) <= 16 * values.len());
        assert!(core::mem::size_of::<ContentIndex<'_>>() <= 15 * 1024);
    }
    #[test]
    fn bounded_index_matches_linear_equality_for_shuffled_repeated_keys() {
        let values = [("b", "2", 8), ("a", "1", 3), ("a", "1", 9), ("a", "0", 4)];
        let index = Index::<_, 4>::new(&values, |v| (v.0, v.1)).unwrap();
        for a in ["a", "b", "c"] {
            for b in ["0", "1", "2"] {
                for value in 0..12 {
                    let query = (a, b, value);
                    assert_eq!(index.contains(&query), values.contains(&query));
                }
            }
        }
        assert!(Index::<_, 3>::new(&values, |v| (v.0, v.1)).is_none());
        let empty = Index::<_, 0>::new(&[] as &[(&str, &str, u8)], |v| (v.0, v.1)).unwrap();
        assert!(!empty.contains(&("a", "1", 3)));
    }
}
