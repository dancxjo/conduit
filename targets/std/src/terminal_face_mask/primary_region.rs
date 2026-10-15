//! A unique primary control Region may focus the compact view; Inspect keeps all Face.
use conduit_face::{Face, FaceDisclosureLevel, FaceRelationshipKind, FaceRole, FaceSubject};
use std::collections::BTreeSet;

pub(super) fn root(face: &Face) -> Option<&FaceSubject> {
    let mut regions = face.subjects.iter().filter(|subject| {
        subject.role == FaceRole::Region
            && face.disclosures.iter().any(|item| {
                item.subject == subject.identity && item.level == FaceDisclosureLevel::Primary
            })
    });
    let first = regions.next()?;
    regions.next().is_none().then_some(first)
}

pub(super) fn scope(face: &Face) -> Option<BTreeSet<&str>> {
    let root = root(face)?;
    let mut scope = BTreeSet::from([root.identity.as_str()]);
    for _ in 0..face.subjects.len() {
        let mut changed = false;
        for relation in &face.relationships {
            if relation.kind == FaceRelationshipKind::Contains
                && scope.contains(relation.source.as_str())
            {
                changed |= scope.insert(relation.target.as_str());
            }
        }
        if !changed {
            break;
        }
    }
    Some(scope)
}
