//! Exact ascending-distance V4 matching through a bounded byte-class index.
use super::{MATCH, NEAR_WINDOW, V4_HISTORY, WINDOW};
#[path = "native_restart_match_index.rs"]
mod index;
use index::Candidates;
pub(super) use index::Index;
fn length(
    history: &[u8; WINDOW],
    look: &[u8; MATCH],
    look_len: usize,
    offset: usize,
    distance: usize,
    at: usize,
) -> usize {
    // A first-byte mismatch is by far the common case. In lazy matching the
    // virtual consumed byte at distance one is look[0], not unwritten history.
    let first = if offset >= distance {
        look[offset - distance]
    } else {
        history[at]
    };
    if first != look[offset] {
        return 0;
    }
    let mut length = 1;
    while length + offset < look_len {
        let byte = if length + offset >= distance {
            look[length + offset - distance]
        } else {
            // at < HISTORY and length < MATCH, so at+length wraps at most once.
            let position = at + length;
            history[if position >= V4_HISTORY {
                position - V4_HISTORY
            } else {
                position
            }]
        };
        if byte != look[length + offset] {
            break;
        }
        length += 1;
    }
    length
}
pub(super) fn longest(
    index: &Index,
    history: &[u8; WINDOW],
    look: &[u8; MATCH],
    look_len: usize,
    consumed: usize,
) -> (usize, usize, usize, usize) {
    let (mut best, mut best_distance, mut near, mut near_distance) = (0, 0, 0, 0);
    for (distance, at) in
        Candidates::new(index.bucket(look[0]), consumed, consumed.min(V4_HISTORY), 1)
    {
        let length = length(history, look, look_len, 0, distance, at);
        if distance <= NEAR_WINDOW && length > near {
            near = length;
            near_distance = distance;
        }
        if length > best {
            best = length;
            best_distance = distance;
        }
        if best == look_len {
            break;
        }
    }
    (best, best_distance, near, near_distance)
}
pub(super) fn lazy(
    index: &Index,
    history: &[u8; WINDOW],
    look: &[u8; MATCH],
    look_len: usize,
    consumed: usize,
    ordinary_used: usize,
    ordinary_cost: usize,
) -> bool {
    // Distance one is the virtual byte that lazy evaluation has not stored.
    let first = length(history, look, look_len, 1, 1, 0);
    if first >= 3 && (first + 1) * ordinary_cost > ordinary_used * 3 {
        return true;
    }
    for (distance, at) in Candidates::new(
        index.bucket(look[1]),
        consumed + 1,
        (consumed + 1).min(V4_HISTORY),
        2,
    ) {
        let length = length(history, look, look_len, 1, distance, at);
        let cost = if distance <= NEAR_WINDOW { 2 } else { 4 };
        if length >= 3 && (length + 1) * ordinary_cost > ordinary_used * (cost + 1) {
            return true;
        }
    }
    false
}
