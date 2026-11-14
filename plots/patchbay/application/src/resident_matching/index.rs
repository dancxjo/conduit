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
/// Packed fingerprint/u16 offsets into one immutable borrowed slice.
/// Fingerprints only select candidates; full key and value equality decide membership.
/// Integer sorting avoids repeatedly traversing long shared identity prefixes.
pub(super) struct Index<'a, T, const N: usize> {
    values: &'a [T],
    order: [u32; N],
    key: fn(&T) -> (&str, &str),
}
impl<'a, T: PartialEq, const N: usize> Index<'a, T, N> {
    fn new(values: &'a [T], key: fn(&T) -> (&str, &str)) -> Option<Self> {
        if values.len() > N || values.len() > usize::from(u16::MAX) {
            return None;
        }
        let mut order = [0; N];
        let mut prefixes = FingerprintPrefix::new();
        for (i, item) in order[..values.len()].iter_mut().enumerate() {
            *item = (u32::from(prefixes.fingerprint(key(&values[i])))) << 16 | i as u32;
        }
        order[..values.len()].sort_unstable();
        Some(Self { values, order, key })
    }
    /// One immutable checkpoint is compared without rehashing adjacent primary
    /// keys. This scratch borrows only this invocation's slice; it never survives
    /// a revision, publication, or mutation of the authoritative projection.
    pub fn contains_all(&self, values: &[T]) -> bool {
        let mut prefixes = FingerprintPrefix::new();
        values.iter().all(|value| {
            let key = (self.key)(value);
            self.contains_fingerprint(value, key, prefixes.fingerprint(key))
        })
    }
    #[cfg(test)]
    fn contains(&self, value: &T) -> bool {
        let key = (self.key)(value);
        self.contains_fingerprint(value, key, fingerprint(key))
    }
    fn contains_fingerprint(&self, value: &T, key: (&str, &str), fingerprint: u16) -> bool {
        let order = &self.order[..self.values.len()];
        let fingerprint = u32::from(fingerprint);
        let first = order.partition_point(|&entry| entry >> 16 < fingerprint);
        order[first..]
            .iter()
            .take_while(|&&entry| entry >> 16 == fingerprint)
            .map(|&entry| &self.values[usize::from(entry as u16)])
            .any(|candidate| (self.key)(candidate) == key && candidate == value)
    }
}

/// FNV state after the complete first key and its original separator. Reuse
/// requires exact borrowed-string equality, never a fingerprint or pointer match.
struct FingerprintPrefix<'a> {
    primary: Option<&'a str>,
    state: u32,
    #[cfg(test)]
    hashed_bytes: usize,
}
impl<'a> FingerprintPrefix<'a> {
    fn new() -> Self {
        Self {
            primary: None,
            state: 0,
            #[cfg(test)]
            hashed_bytes: 0,
        }
    }
    fn fingerprint(&mut self, key: (&'a str, &str)) -> u16 {
        if self.primary != Some(key.0) {
            let mut hash = 2_166_136_261u32;
            for byte in key.0.bytes().chain(core::iter::once(255)) {
                hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
                #[cfg(test)]
                {
                    self.hashed_bytes += 1;
                }
            }
            self.primary = Some(key.0);
            self.state = hash;
        }
        let mut hash = self.state;
        for byte in key.1.bytes().chain(core::iter::once(255)) {
            hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
            #[cfg(test)]
            {
                self.hashed_bytes += 1;
            }
        }
        (hash ^ (hash >> 16)) as u16
    }
}
// Independent original scalar definition, retained as the exact test oracle.
#[cfg(test)]
fn fingerprint(key: (&str, &str)) -> u16 {
    let mut hash = 2_166_136_261u32;
    for part in [key.0, key.1] {
        for byte in part.bytes().chain(core::iter::once(255)) {
            hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
        }
    }
    (hash ^ (hash >> 16)) as u16
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn index_workspace_and_distinct_key_lookup_work_are_bounded() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        static KEYS: AtomicUsize = AtomicUsize::new(0);
        fn key(value: &(alloc::string::String, u8)) -> (&str, &str) {
            KEYS.fetch_add(1, Ordering::Relaxed);
            (value.0.as_str(), "")
        }
        let values = (0..1024)
            .rev()
            .map(|i| (alloc::format!("{i:04}"), 0u8))
            .collect::<alloc::vec::Vec<_>>();
        let index = Index::<_, 1024>::new(&values, key).unwrap();
        assert_eq!(KEYS.load(Ordering::Relaxed), values.len());
        KEYS.store(0, Ordering::Relaxed);
        for value in &values {
            assert!(index.contains(value));
        }
        assert!(KEYS.load(Ordering::Relaxed) <= 4 * values.len());
        assert!(core::mem::size_of::<ContentIndex<'_>>() <= 29 * 1024);
    }
    #[test]
    fn fingerprint_collisions_preserve_exact_keys_and_values() {
        let first = ("key/124", "", 3);
        let collision = ("key/336", "", 9);
        assert_eq!(
            fingerprint((first.0, first.1)),
            fingerprint((collision.0, collision.1))
        );
        let values = [first, collision, ("key/124", "", 7)];
        let index = Index::<_, 3>::new(&values, |v| (v.0, v.1)).unwrap();
        for key in ["key/124", "key/336", "absent"] {
            for value in 0..12 {
                let query = (key, "", value);
                assert_eq!(index.contains(&query), values.contains(&query));
            }
        }
        let one = Index::<_, 1>::new(&values[..1], |v| (v.0, v.1)).unwrap();
        assert!(!one.contains(&("key/336", "", 3)));
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

#[cfg(test)]
#[path = "index_prefix_tests.rs"]
mod prefix_tests;
