//! Resolve declared rhetorical hierarchy into the native reader's first view.
//!
//! This is presentation policy over immutable Face truth. Associated content
//! within a containing subject leads its structural context; subordinate text
//! remains in the complete Details reader. Neither choice changes semantic
//! order, action identity, availability, or image-resource selection.
use super::{Item, layout};
use alloc::vec::Vec;
use conduit_face::{Face, FaceCompositionKind, FaceRelationshipKind};

pub(super) fn arrange_primary(face: &Face, items: &mut Vec<Item>) {
    let index = |identity: &str| {
        face.subjects
            .iter()
            .position(|subject| subject.identity == identity)
    };
    let contains: Vec<_> = face
        .relationships
        .iter()
        .filter(|relation| relation.kind == FaceRelationshipKind::Contains)
        .filter_map(|relation| Some((index(&relation.source)?, index(&relation.target)?)))
        .collect();
    let mut associated = alloc::vec![false; face.subjects.len()];
    let mut subordinate = alloc::vec![false; face.subjects.len()];
    for relation in &face.composition {
        let (Some(child), Some(parent)) = (index(&relation.source), index(&relation.target)) else {
            continue;
        };
        // A peer association is not a containment hierarchy. Do not invent a
        // hierarchy from wording, roles, serialization order or identity names.
        if !contains.contains(&(parent, child)) {
            continue;
        }
        match relation.kind {
            FaceCompositionKind::Associate => associated[child] = true,
            FaceCompositionKind::Subordinate => subordinate[child] = true,
            _ => {}
        }
    }
    // Finite transitive closure, including cycles and multiple parents. Subject
    // and relationship counts inherit the already validated Face bounds.
    for _ in 0..face.subjects.len() {
        let mut changed = false;
        for &(parent, child) in &contains {
            if associated[parent] && !associated[child] {
                associated[child] = true;
                changed = true;
            }
            if subordinate[parent] && !subordinate[child] {
                subordinate[child] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Keep every control in the primary traversal as well as Details. No
    // action is made inaccessible merely because its subject is subordinate.
    items.retain(|item| {
        item.part == layout::NativePart::Control
            || !item.subject_index.is_some_and(|index| subordinate[index])
    });
    // Stable partition without a second Item buffer. Existing order within
    // each block, exact control indices and subject correlations are retained.
    let mut first_context = 0;
    for current in 0..items.len() {
        if items[current]
            .subject_index
            .is_some_and(|index| associated[index] && !subordinate[index])
        {
            items[first_context..=current].rotate_right(1);
            first_context += 1;
        }
    }
}

#[cfg(test)]
#[path = "hierarchy_tests.rs"]
mod tests;
