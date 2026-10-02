//! Finite source expressions for portable checked text patterns.
//!
//! Checking lowers these expressions once. Play only receives the deterministic
//! automaton in [`CheckedTextPattern`]; it never interprets source syntax.

use crate::prelude::*;
use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use conduit_core::{
    CheckedTextPattern, ConstraintDefinitionError, TextPatternState, TextPatternTransition,
    MAX_PATTERN_MATCH_STEPS, MAX_PATTERN_STATES, MAX_PATTERN_TRANSITIONS,
};

const MAX_SCALAR: u32 = 0x10_ffff;
const SURROGATE_FIRST: u32 = 0xd800;
const SURROGATE_LAST: u32 = 0xdfff;
// Thompson states are temporary checking data, but must remain bounded too.
const MAX_NFA_STATES: usize = MAX_PATTERN_TRANSITIONS * 2 + 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPatternExpression {
    Empty,
    ScalarRange {
        first: u32,
        last: u32,
    },
    Sequence(Vec<TextPatternExpression>),
    Choice(Vec<TextPatternExpression>),
    Repeat {
        expression: Box<TextPatternExpression>,
        minimum: u16,
        maximum: u16,
    },
    InputBoundRepeat {
        expression: Box<TextPatternExpression>,
        minimum: u16,
    },
    PrefixAssertion {
        assertion: Box<TextPatternExpression>,
        remainder: Box<TextPatternExpression>,
        negated: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPatternDefinitionError {
    InvalidScalarRange { first: u32, last: u32 },
    InvalidRepeatRange { minimum: u16, maximum: u16 },
    ExpressionTooComplex,
    TooManyStates,
    TooManyTransitions,
    MatchWorkExceeded,
    InvalidCompiledPattern(ConstraintDefinitionError),
}

#[derive(Default)]
struct NfaState {
    epsilon: Vec<usize>,
    transitions: Vec<NfaTransition>,
}

struct NfaTransition {
    first: u32,
    last: u32,
    target: usize,
}

#[derive(Clone, Copy)]
struct Fragment {
    start: usize,
    end: usize,
}

struct Nfa {
    states: Vec<NfaState>,
}

impl TextPatternExpression {
    /// Applies Conduit's portable `i` profile: ASCII letters compare without
    /// case while all other Unicode scalars retain their exact identity.
    pub fn ascii_case_insensitive(&self) -> Self {
        match self {
            Self::Empty => Self::Empty,
            Self::ScalarRange { first, last } => {
                let mut ranges = vec![Self::ScalarRange {
                    first: *first,
                    last: *last,
                }];
                let upper_first = (*first).max(u32::from(b'A'));
                let upper_last = (*last).min(u32::from(b'Z'));
                if upper_first <= upper_last {
                    ranges.push(Self::ScalarRange {
                        first: upper_first + 32,
                        last: upper_last + 32,
                    });
                }
                let lower_first = (*first).max(u32::from(b'a'));
                let lower_last = (*last).min(u32::from(b'z'));
                if lower_first <= lower_last {
                    ranges.push(Self::ScalarRange {
                        first: lower_first - 32,
                        last: lower_last - 32,
                    });
                }
                if ranges.len() == 1 {
                    ranges.pop().expect("the exact source range remains")
                } else {
                    Self::Choice(ranges)
                }
            }
            Self::Sequence(expressions) => Self::Sequence(
                expressions
                    .iter()
                    .map(Self::ascii_case_insensitive)
                    .collect(),
            ),
            Self::Choice(expressions) => Self::Choice(
                expressions
                    .iter()
                    .map(Self::ascii_case_insensitive)
                    .collect(),
            ),
            Self::Repeat {
                expression,
                minimum,
                maximum,
            } => Self::Repeat {
                expression: Box::new(expression.ascii_case_insensitive()),
                minimum: *minimum,
                maximum: *maximum,
            },
            Self::InputBoundRepeat {
                expression,
                minimum,
            } => Self::InputBoundRepeat {
                expression: Box::new(expression.ascii_case_insensitive()),
                minimum: *minimum,
            },
            Self::PrefixAssertion {
                assertion,
                remainder,
                negated,
            } => Self::PrefixAssertion {
                assertion: Box::new(assertion.ascii_case_insensitive()),
                remainder: Box::new(remainder.ascii_case_insensitive()),
                negated: *negated,
            },
        }
    }

    pub fn compile(
        &self,
        maximum_input_characters: u32,
    ) -> Result<CheckedTextPattern, TextPatternDefinitionError> {
        if let Self::PrefixAssertion {
            assertion,
            remainder,
            negated,
        } = self
        {
            let assertion = assertion.compile(maximum_input_characters)?;
            let remainder = remainder.compile(maximum_input_characters)?;
            return crate::value_pattern_lookahead::compile_prefix_assertion(
                &assertion, &remainder, *negated,
            );
        }
        let bounded = self.bind_input_repetition(maximum_input_characters)?;
        let mut nfa = Nfa { states: Vec::new() };
        let fragment = nfa.build(&bounded)?;
        determinize(&nfa, fragment, maximum_input_characters)
    }

    fn bind_input_repetition(
        &self,
        maximum_input_characters: u32,
    ) -> Result<Self, TextPatternDefinitionError> {
        match self {
            Self::Empty | Self::ScalarRange { .. } => Ok(self.clone()),
            Self::Sequence(expressions) => expressions
                .iter()
                .map(|expression| expression.bind_input_repetition(maximum_input_characters))
                .collect::<Result<Vec<_>, _>>()
                .map(Self::Sequence),
            Self::Choice(expressions) => expressions
                .iter()
                .map(|expression| expression.bind_input_repetition(maximum_input_characters))
                .collect::<Result<Vec<_>, _>>()
                .map(Self::Choice),
            Self::Repeat {
                expression,
                minimum,
                maximum,
            } => Ok(Self::Repeat {
                expression: Box::new(expression.bind_input_repetition(maximum_input_characters)?),
                minimum: *minimum,
                maximum: *maximum,
            }),
            Self::InputBoundRepeat {
                expression,
                minimum,
            } => {
                let maximum = u16::try_from(maximum_input_characters)
                    .map_err(|_| TextPatternDefinitionError::ExpressionTooComplex)?;
                if *minimum > maximum {
                    return Err(TextPatternDefinitionError::InvalidRepeatRange {
                        minimum: *minimum,
                        maximum,
                    });
                }
                Ok(Self::Repeat {
                    expression: Box::new(
                        expression.bind_input_repetition(maximum_input_characters)?,
                    ),
                    minimum: *minimum,
                    maximum,
                })
            }
            Self::PrefixAssertion { .. } => {
                unreachable!("assertions compile before repetition binding")
            }
        }
    }

    pub fn compile_search(
        &self,
        maximum_input_characters: u32,
    ) -> Result<CheckedTextPattern, TextPatternDefinitionError> {
        let pattern = self.compile(maximum_input_characters)?;
        let maximum_fanout = pattern
            .states
            .iter()
            .map(|state| state.transitions.len() as u32)
            .max()
            .unwrap_or(0)
            .max(1);
        let starts = maximum_input_characters
            .checked_add(1)
            .ok_or(TextPatternDefinitionError::MatchWorkExceeded)?;
        let character_visits = maximum_input_characters
            .checked_mul(starts)
            .and_then(|value| value.checked_div(2))
            .ok_or(TextPatternDefinitionError::MatchWorkExceeded)?;
        let maximum_match_steps = character_visits
            .checked_mul(maximum_fanout)
            .filter(|steps| *steps <= MAX_PATTERN_MATCH_STEPS)
            .ok_or(TextPatternDefinitionError::MatchWorkExceeded)?;
        CheckedTextPattern::new(
            pattern.states,
            pattern.start_state,
            pattern.maximum_input_characters,
            maximum_match_steps,
        )
        .map_err(TextPatternDefinitionError::InvalidCompiledPattern)
    }
}

impl Nfa {
    fn state(&mut self) -> Result<usize, TextPatternDefinitionError> {
        if self.states.len() >= MAX_NFA_STATES {
            return Err(TextPatternDefinitionError::ExpressionTooComplex);
        }
        let index = self.states.len();
        self.states.push(NfaState::default());
        Ok(index)
    }

    fn empty(&mut self) -> Result<Fragment, TextPatternDefinitionError> {
        let start = self.state()?;
        let end = self.state()?;
        self.states[start].epsilon.push(end);
        Ok(Fragment { start, end })
    }

    fn build(
        &mut self,
        expression: &TextPatternExpression,
    ) -> Result<Fragment, TextPatternDefinitionError> {
        match expression {
            TextPatternExpression::Empty => self.empty(),
            TextPatternExpression::ScalarRange { first, last } => {
                if first > last
                    || *last > MAX_SCALAR
                    || char::from_u32(*first).is_none()
                    || char::from_u32(*last).is_none()
                {
                    return Err(TextPatternDefinitionError::InvalidScalarRange {
                        first: *first,
                        last: *last,
                    });
                }
                let start = self.state()?;
                let end = self.state()?;
                if *first < SURROGATE_FIRST && *last > SURROGATE_LAST {
                    self.states[start].transitions.push(NfaTransition {
                        first: *first,
                        last: SURROGATE_FIRST - 1,
                        target: end,
                    });
                    self.states[start].transitions.push(NfaTransition {
                        first: SURROGATE_LAST + 1,
                        last: *last,
                        target: end,
                    });
                } else {
                    self.states[start].transitions.push(NfaTransition {
                        first: *first,
                        last: *last,
                        target: end,
                    });
                }
                Ok(Fragment { start, end })
            }
            TextPatternExpression::Sequence(expressions) => {
                let Some((first, rest)) = expressions.split_first() else {
                    return self.empty();
                };
                let mut result = self.build(first)?;
                for expression in rest {
                    let next = self.build(expression)?;
                    self.states[result.end].epsilon.push(next.start);
                    result.end = next.end;
                }
                Ok(result)
            }
            TextPatternExpression::Choice(expressions) => {
                let start = self.state()?;
                let end = self.state()?;
                for expression in expressions {
                    let branch = self.build(expression)?;
                    self.states[start].epsilon.push(branch.start);
                    self.states[branch.end].epsilon.push(end);
                }
                Ok(Fragment { start, end })
            }
            TextPatternExpression::Repeat {
                expression,
                minimum,
                maximum,
            } => self.repeat(expression, *minimum, *maximum),
            TextPatternExpression::InputBoundRepeat { .. }
            | TextPatternExpression::PrefixAssertion { .. } => {
                Err(TextPatternDefinitionError::ExpressionTooComplex)
            }
        }
    }

    fn repeat(
        &mut self,
        expression: &TextPatternExpression,
        minimum: u16,
        maximum: u16,
    ) -> Result<Fragment, TextPatternDefinitionError> {
        if minimum > maximum {
            return Err(TextPatternDefinitionError::InvalidRepeatRange { minimum, maximum });
        }
        let result = self.empty()?;
        let mut end = result.end;
        for _ in 0..minimum {
            let copy = self.build(expression)?;
            self.states[end].epsilon.push(copy.start);
            end = copy.end;
        }
        for _ in minimum..maximum {
            let optional_end = self.state()?;
            let copy = self.build(expression)?;
            self.states[end].epsilon.push(optional_end);
            self.states[end].epsilon.push(copy.start);
            self.states[copy.end].epsilon.push(optional_end);
            end = optional_end;
        }
        Ok(Fragment {
            start: result.start,
            end,
        })
    }
}

fn epsilon_closure(nfa: &Nfa, seeds: impl IntoIterator<Item = usize>) -> BTreeSet<usize> {
    let mut closure = BTreeSet::new();
    let mut pending: Vec<usize> = seeds.into_iter().collect();
    while let Some(state) = pending.pop() {
        if closure.insert(state) {
            pending.extend(nfa.states[state].epsilon.iter().copied());
        }
    }
    closure
}

fn determinize(
    nfa: &Nfa,
    fragment: Fragment,
    maximum_input_characters: u32,
) -> Result<CheckedTextPattern, TextPatternDefinitionError> {
    let initial = epsilon_closure(nfa, [fragment.start]);
    let mut indices = BTreeMap::new();
    indices.insert(initial.clone(), 0_u16);
    let mut pending = VecDeque::from([initial]);
    let mut states = Vec::new();
    let mut transition_count = 0_usize;

    while let Some(subset) = pending.pop_front() {
        if states.len() >= MAX_PATTERN_STATES {
            return Err(TextPatternDefinitionError::TooManyStates);
        }
        let mut boundaries = BTreeSet::new();
        for state in &subset {
            for transition in &nfa.states[*state].transitions {
                boundaries.insert(transition.first);
                if transition.last < MAX_SCALAR {
                    boundaries.insert(transition.last + 1);
                }
            }
        }
        let points: Vec<u32> = boundaries.into_iter().collect();
        let mut transitions: Vec<TextPatternTransition> = Vec::new();
        for (index, first) in points.iter().copied().enumerate() {
            if char::from_u32(first).is_none() {
                continue;
            }
            let next_boundary = points.get(index + 1).copied().unwrap_or(MAX_SCALAR + 1);
            let last = next_boundary.saturating_sub(1).min(MAX_SCALAR);
            let targets = subset
                .iter()
                .flat_map(|state| nfa.states[*state].transitions.iter())
                .filter(|transition| first >= transition.first && first <= transition.last)
                .map(|transition| transition.target);
            let target = epsilon_closure(nfa, targets);
            if target.is_empty() {
                continue;
            }
            let target_index = if let Some(index) = indices.get(&target) {
                *index
            } else {
                if indices.len() >= MAX_PATTERN_STATES {
                    return Err(TextPatternDefinitionError::TooManyStates);
                }
                let index = u16::try_from(indices.len())
                    .map_err(|_| TextPatternDefinitionError::TooManyStates)?;
                indices.insert(target.clone(), index);
                pending.push_back(target);
                index
            };
            if let Some(previous) = transitions.last_mut() {
                if previous.target_state == target_index
                    && previous.last_scalar.checked_add(1) == Some(first)
                {
                    previous.last_scalar = last;
                    continue;
                }
            }
            transition_count += 1;
            if transition_count > MAX_PATTERN_TRANSITIONS {
                return Err(TextPatternDefinitionError::TooManyTransitions);
            }
            transitions.push(TextPatternTransition {
                first_scalar: first,
                last_scalar: last,
                target_state: target_index,
            });
        }
        states.push(TextPatternState {
            accepting: subset.contains(&fragment.end),
            transitions,
        });
    }

    let maximum_fanout = states
        .iter()
        .map(|state| state.transitions.len() as u32)
        .max()
        .unwrap_or(0)
        .max(1);
    let maximum_match_steps = maximum_input_characters
        .checked_mul(maximum_fanout)
        .filter(|steps| *steps <= MAX_PATTERN_MATCH_STEPS)
        .ok_or(TextPatternDefinitionError::MatchWorkExceeded)?;
    CheckedTextPattern::new(states, 0, maximum_input_characters, maximum_match_steps)
        .map_err(TextPatternDefinitionError::InvalidCompiledPattern)
}
