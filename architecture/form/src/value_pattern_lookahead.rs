//! Finite deterministic construction for leading portable lookahead.

use crate::prelude::*;
use crate::TextPatternDefinitionError;
use alloc::collections::{BTreeMap, VecDeque};
use conduit_core::{
    CheckedTextPattern, TextPatternState, TextPatternTransition, MAX_PATTERN_MATCH_STEPS,
    MAX_PATTERN_STATES, MAX_PATTERN_TRANSITIONS,
};

const AFTER_MAX_SCALAR: u32 = 0x11_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ProductState {
    remainder: u16,
    assertion: Option<u16>,
    matched: bool,
}

pub(super) fn compile_prefix_assertion(
    assertion: &CheckedTextPattern,
    remainder: &CheckedTextPattern,
    negated: bool,
) -> Result<CheckedTextPattern, TextPatternDefinitionError> {
    let assertion_accepts_empty = assertion.states[usize::from(assertion.start_state)].accepting;
    let initial = ProductState {
        remainder: remainder.start_state,
        assertion: (!assertion_accepts_empty).then_some(assertion.start_state),
        matched: assertion_accepts_empty,
    };
    let mut indices = BTreeMap::from([(initial, 0_u16)]);
    let mut pending = VecDeque::from([initial]);
    let mut states = Vec::new();
    let mut transition_count = 0_usize;

    while let Some(product) = pending.pop_front() {
        if states.len() >= MAX_PATTERN_STATES {
            return Err(TextPatternDefinitionError::TooManyStates);
        }
        let remainder_state = &remainder.states[usize::from(product.remainder)];
        let assertion_state = product
            .assertion
            .map(|state| &assertion.states[usize::from(state)]);
        let mut boundaries = vec![AFTER_MAX_SCALAR];
        for transition in remainder_state.transitions.iter().chain(
            assertion_state
                .into_iter()
                .flat_map(|state| &state.transitions),
        ) {
            boundaries.push(transition.first_scalar);
            boundaries.push(transition.last_scalar.saturating_add(1));
        }
        boundaries.sort_unstable();
        boundaries.dedup();

        let mut transitions: Vec<TextPatternTransition> = Vec::new();
        for pair in boundaries.windows(2) {
            let first = pair[0];
            if first >= AFTER_MAX_SCALAR {
                continue;
            }
            let last = pair[1] - 1;
            let Some(remainder_target) = target(remainder_state, first) else {
                continue;
            };
            let assertion_target = assertion_state.and_then(|state| target(state, first));
            let matched = product.matched
                || assertion_target
                    .is_some_and(|state| assertion.states[usize::from(state)].accepting);
            let next = ProductState {
                remainder: remainder_target,
                assertion: if matched { None } else { assertion_target },
                matched,
            };
            let next_index = match indices.get(&next) {
                Some(index) => *index,
                None => {
                    let index = u16::try_from(indices.len())
                        .map_err(|_| TextPatternDefinitionError::TooManyStates)?;
                    if usize::from(index) >= MAX_PATTERN_STATES {
                        return Err(TextPatternDefinitionError::TooManyStates);
                    }
                    indices.insert(next, index);
                    pending.push_back(next);
                    index
                }
            };
            if let Some(previous) = transitions.last_mut() {
                if previous.target_state == next_index
                    && previous.last_scalar.checked_add(1) == Some(first)
                {
                    previous.last_scalar = last;
                    continue;
                }
            }
            transitions.push(TextPatternTransition {
                first_scalar: first,
                last_scalar: last,
                target_state: next_index,
            });
            transition_count += 1;
            if transition_count > MAX_PATTERN_TRANSITIONS {
                return Err(TextPatternDefinitionError::TooManyTransitions);
            }
        }
        states.push(TextPatternState {
            accepting: remainder_state.accepting && (product.matched != negated),
            transitions,
        });
    }

    let maximum_fanout = states
        .iter()
        .map(|state| state.transitions.len() as u32)
        .max()
        .unwrap_or(0)
        .max(1);
    let maximum_match_steps = remainder
        .maximum_input_characters
        .checked_mul(maximum_fanout)
        .filter(|steps| *steps <= MAX_PATTERN_MATCH_STEPS)
        .ok_or(TextPatternDefinitionError::MatchWorkExceeded)?;
    CheckedTextPattern::new(
        states,
        0,
        remainder.maximum_input_characters,
        maximum_match_steps,
    )
    .map_err(TextPatternDefinitionError::InvalidCompiledPattern)
}

fn target(state: &TextPatternState, scalar: u32) -> Option<u16> {
    state
        .transitions
        .iter()
        .find(|transition| scalar >= transition.first_scalar && scalar <= transition.last_scalar)
        .map(|transition| transition.target_state)
}
