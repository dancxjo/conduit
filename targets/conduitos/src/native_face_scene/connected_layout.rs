//! Finite renderer ordering of exact Face connections; no semantic graph edits.
use conduit_presentation::{Presentation, PresentationRelationshipKind, PresentationRole};

pub(super) const MAX_GEARS: usize = 64;
const MAX_CORDS: usize = 128;

pub(super) struct Order {
    pub subjects: [usize; MAX_GEARS],
    pub count: usize,
    pub fallback: bool,
    pub cycle_breaks: usize,
    pub unresolved: usize,
}

impl Order {
    pub fn label(&self) -> &'static str {
        if self.fallback {
            "Grid bound"
        } else if self.cycle_breaks != 0 {
            "Cycles"
        } else if self.unresolved != 0 {
            "Linked; unresolved"
        } else {
            "Linked"
        }
    }
}

pub(super) fn order(face: &Presentation) -> Order {
    let mut result = Order {
        subjects: [0; MAX_GEARS],
        count: 0,
        fallback: false,
        cycle_breaks: 0,
        unresolved: 0,
    };
    let count = face
        .subjects
        .iter()
        .filter(|s| s.role == PresentationRole::Gear)
        .count();
    let cords = face
        .subjects
        .iter()
        .filter(|s| s.role == PresentationRole::Cord)
        .count();
    if count > MAX_GEARS || cords > MAX_CORDS {
        result.fallback = true;
        return result;
    }
    let mut gears = [0; MAX_GEARS];
    for (index, (subject, _)) in face
        .subjects
        .iter()
        .enumerate()
        .filter(|(_, s)| s.role == PresentationRole::Gear)
        .enumerate()
    {
        gears[index] = subject;
    }
    let mut outgoing = [0u64; MAX_GEARS];
    let mut incoming = [0u64; MAX_GEARS];
    for cord in face
        .subjects
        .iter()
        .filter(|s| s.role == PresentationRole::Cord)
    {
        let endpoints = super::ports::endpoint(face, cord, "source-port", "outgoing")
            .zip(super::ports::endpoint(face, cord, "sink-port", "receiving"));
        let pair = endpoints.and_then(|(source, sink)| {
            index_for_port(face, &gears[..count], &source.identity).zip(index_for_port(
                face,
                &gears[..count],
                &sink.identity,
            ))
        });
        if let Some((source, sink)) = pair {
            outgoing[source] |= 1u64 << sink;
            incoming[sink] |= 1u64 << source;
        } else {
            result.unresolved += 1;
        }
    }
    // Weak components stay together. Each queue and adjacency row is a fixed
    // bitmap; every node is visited once, with at most 64 scans per phase.
    let mut visited = 0u64;
    for root in 0..count {
        let bit = 1u64 << root;
        if visited & bit != 0 {
            continue;
        }
        let mut component = bit;
        let mut pending = bit;
        while pending != 0 {
            let node = pending.trailing_zeros() as usize;
            pending &= !(1u64 << node);
            let added = (outgoing[node] | incoming[node]) & !component;
            component |= added;
            pending |= added;
        }
        visited |= component;
        let mut remaining = component;
        while remaining != 0 {
            let node = (0..count)
                .find(|&node| remaining & (1u64 << node) != 0 && incoming[node] & remaining == 0)
                .unwrap_or_else(|| {
                    // Break only the geometry ordering of a cycle, never a Cord.
                    result.cycle_breaks += 1;
                    remaining.trailing_zeros() as usize
                });
            result.subjects[result.count] = gears[node];
            result.count += 1;
            remaining &= !(1u64 << node);
        }
    }
    result
}

fn index_for_port(face: &Presentation, gears: &[usize], port: &str) -> Option<usize> {
    let owner = unique_owner(face, port)?;
    gears.iter().position(|&subject| subject == owner)
}

/// Ownership ambiguity must remain unresolved even when one owner is off-page.
pub(super) fn unique_owner(face: &Presentation, port: &str) -> Option<usize> {
    let mut owner = None;
    for relation in face
        .relationships
        .iter()
        .filter(|r| r.kind == PresentationRelationshipKind::Contains && r.target == port)
    {
        let Some(index) = face
            .subjects
            .iter()
            .position(|s| s.role == PresentationRole::Gear && s.identity == relation.source)
        else {
            continue;
        };
        if owner.is_some_and(|previous| previous != index) {
            return None;
        }
        owner = Some(index);
    }
    owner
}

#[cfg(test)]
#[path = "connected_layout/tests.rs"]
mod tests;
