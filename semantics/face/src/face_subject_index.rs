//! Temporary exact lookup for subject validation within one borrowed Face.

use super::{FaceError, FaceSubject, MAX_FACE_SUBJECTS};

pub(super) struct SubjectIndex<'a> {
    subjects: &'a [FaceSubject],
    order: [u16; MAX_FACE_SUBJECTS],
}

impl<'a> SubjectIndex<'a> {
    pub(super) fn new(subjects: &'a [FaceSubject]) -> Result<Self, FaceError> {
        let mut order = [0; MAX_FACE_SUBJECTS];
        let order_slice = order
            .get_mut(..subjects.len())
            .ok_or(FaceError::TooManySubjects)?;
        let identity = |index: u16| subjects[usize::from(index)].identity.as_str();
        for (index, entry) in order_slice.iter_mut().enumerate() {
            *entry = index as u16;
        }
        order_slice.sort_unstable_by_key(|&index| identity(index));
        if order_slice
            .windows(2)
            .any(|pair| identity(pair[0]) == identity(pair[1]))
        {
            return Err(FaceError::DuplicateSubject);
        }
        Ok(Self { subjects, order })
    }

    pub(super) fn contains(&self, identity: &str) -> bool {
        self.order[..self.subjects.len()]
            .binary_search_by(|&index| {
                self.subjects[usize::from(index)]
                    .identity
                    .as_str()
                    .cmp(identity)
            })
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn full_index_matches_exact_subjects_and_refuses_duplicate_ids() {
        let mut subjects = (0..MAX_FACE_SUBJECTS)
            .rev()
            .map(|ordinal| FaceSubject {
                identity: alloc::format!("body/shared-prefix/{ordinal:04}"),
                role: super::super::FaceRole::Info,
                name: alloc::format!("Subject {ordinal}"),
            })
            .collect::<Vec<_>>();
        let index = SubjectIndex::new(&subjects).unwrap();
        for subject in &subjects {
            assert!(index.contains(&subject.identity));
        }
        assert!(!index.contains("body/shared-prefix/000"));
        assert!(!index.contains("body/shared-prefix/1024"));

        let duplicate = subjects[0].identity.clone();
        subjects[MAX_FACE_SUBJECTS - 1].identity = duplicate;
        assert!(matches!(
            SubjectIndex::new(&subjects),
            Err(FaceError::DuplicateSubject)
        ));
    }
}
