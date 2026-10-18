//! Checked deterministic text-pattern definition and bounded matching.
use super::*;

impl CheckedTextPattern {
    pub fn new(
        states: Vec<TextPatternState>,
        start_state: u16,
        maximum_input_characters: u32,
        maximum_match_steps: u32,
    ) -> Result<Self, ConstraintDefinitionError> {
        let pattern = Self {
            states,
            start_state,
            maximum_input_characters,
            maximum_match_steps,
        };
        pattern.validate_definition()?;
        Ok(pattern)
    }

    pub(super) fn validate_definition(&self) -> Result<(), ConstraintDefinitionError> {
        if self.states.is_empty() {
            return Err(ConstraintDefinitionError::EmptyPattern);
        }
        if self.states.len() > MAX_PATTERN_STATES {
            return Err(ConstraintDefinitionError::TooManyPatternStates);
        }
        if usize::from(self.start_state) >= self.states.len() {
            return Err(ConstraintDefinitionError::InvalidStartState);
        }
        let transition_count = self
            .states
            .iter()
            .map(|state| state.transitions.len())
            .sum::<usize>();
        if transition_count > MAX_PATTERN_TRANSITIONS {
            return Err(ConstraintDefinitionError::TooManyPatternTransitions);
        }
        let maximum_fanout = self
            .states
            .iter()
            .map(|state| state.transitions.len() as u32)
            .max()
            .unwrap_or(0);
        let required_steps = self
            .maximum_input_characters
            .checked_mul(maximum_fanout.max(1))
            .ok_or(ConstraintDefinitionError::MatchWorkExceeded)?;
        if self.maximum_match_steps < required_steps
            || self.maximum_match_steps > MAX_PATTERN_MATCH_STEPS
        {
            return Err(ConstraintDefinitionError::MatchWorkExceeded);
        }
        for state in &self.states {
            if state
                .transitions
                .windows(2)
                .any(|pair| pair[0].first_scalar >= pair[1].first_scalar)
            {
                return Err(ConstraintDefinitionError::NonCanonicalTransitions);
            }
            for (index, transition) in state.transitions.iter().enumerate() {
                if char::from_u32(transition.first_scalar).is_none()
                    || char::from_u32(transition.last_scalar).is_none()
                    || transition.first_scalar > transition.last_scalar
                {
                    return Err(ConstraintDefinitionError::InvalidTransitionRange);
                }
                if usize::from(transition.target_state) >= self.states.len() {
                    return Err(ConstraintDefinitionError::InvalidTransitionTarget);
                }
                if state.transitions[index + 1..].iter().any(|candidate| {
                    transition.first_scalar <= candidate.last_scalar
                        && candidate.first_scalar <= transition.last_scalar
                }) {
                    return Err(ConstraintDefinitionError::NondeterministicTransitions);
                }
            }
        }
        Ok(())
    }

    pub fn is_match(&self, input: &str) -> bool {
        let mut state = usize::from(self.start_state);
        let mut characters = 0_u32;
        let mut steps = 0_u32;
        for character in input.chars() {
            characters = characters.saturating_add(1);
            if characters > self.maximum_input_characters {
                return false;
            }
            let scalar = character as u32;
            let mut target = None;
            for transition in &self.states[state].transitions {
                steps = steps.saturating_add(1);
                if steps > self.maximum_match_steps {
                    return false;
                }
                if scalar >= transition.first_scalar && scalar <= transition.last_scalar {
                    target = Some(usize::from(transition.target_state));
                    break;
                }
            }
            let Some(next) = target else {
                return false;
            };
            state = next;
        }
        self.states[state].accepting
    }

    /// Returns whether any bounded substring satisfies this deterministic
    /// pattern. Search remains portable checked work: every attempted start
    /// shares the one admitted step budget carried by the pattern.
    pub fn has_match(&self, input: &str) -> bool {
        if input.chars().count() > self.maximum_input_characters as usize {
            return false;
        }
        let mut steps = 0_u32;
        for (start, _) in input
            .char_indices()
            .chain(core::iter::once((input.len(), '\0')))
        {
            let mut state = usize::from(self.start_state);
            if self.states[state].accepting {
                return true;
            }
            for character in input[start..].chars() {
                let scalar = character as u32;
                let mut target = None;
                for transition in &self.states[state].transitions {
                    steps = steps.saturating_add(1);
                    if steps > self.maximum_match_steps {
                        return false;
                    }
                    if scalar >= transition.first_scalar && scalar <= transition.last_scalar {
                        target = Some(usize::from(transition.target_state));
                        break;
                    }
                }
                let Some(next) = target else {
                    break;
                };
                state = next;
                if self.states[state].accepting {
                    return true;
                }
            }
        }
        false
    }

    pub fn has_prefix_match(&self, input: &str) -> bool {
        if input.chars().count() > self.maximum_input_characters as usize {
            return false;
        }
        let mut state = usize::from(self.start_state);
        if self.states[state].accepting {
            return true;
        }
        let mut steps = 0_u32;
        for character in input.chars() {
            let scalar = character as u32;
            let mut target = None;
            for transition in &self.states[state].transitions {
                steps = steps.saturating_add(1);
                if steps > self.maximum_match_steps {
                    return false;
                }
                if scalar >= transition.first_scalar && scalar <= transition.last_scalar {
                    target = Some(usize::from(transition.target_state));
                    break;
                }
            }
            let Some(next) = target else {
                return false;
            };
            state = next;
            if self.states[state].accepting {
                return true;
            }
        }
        false
    }

    pub fn has_suffix_match(&self, input: &str) -> bool {
        if input.chars().count() > self.maximum_input_characters as usize {
            return false;
        }
        let mut steps = 0_u32;
        for offset in input
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(core::iter::once(input.len()))
        {
            let mut state = usize::from(self.start_state);
            for character in input[offset..].chars() {
                let scalar = character as u32;
                let mut target = None;
                for transition in &self.states[state].transitions {
                    steps = steps.saturating_add(1);
                    if steps > self.maximum_match_steps {
                        return false;
                    }
                    if scalar >= transition.first_scalar && scalar <= transition.last_scalar {
                        target = Some(usize::from(transition.target_state));
                        break;
                    }
                }
                let Some(next) = target else {
                    state = usize::MAX;
                    break;
                };
                state = next;
            }
            if state != usize::MAX && self.states[state].accepting {
                return true;
            }
        }
        false
    }
}
