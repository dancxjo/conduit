//! Fixed-resident runtime support for immediate current-value observation.
//!
//! The cell retains exactly one current value. It is not history, persistence,
//! multi-writer arbitration, a CRDT, or mutation authority. Every replacement
//! requires a caller-supplied authorizer before state changes.

/// Metadata presented to the separate mutation-authority boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentUpdateRequest {
    pub current_generation: u64,
    pub next_generation: u64,
}

/// Separate authority check required before a current value can be replaced.
///
/// Implementations may validate the exact typed update port, effect, grant,
/// lease, and time at use. The current-value cell does not manufacture them.
pub trait CurrentValueMutationAuthorizer {
    type Error;

    fn authorize(&mut self, request: CurrentUpdateRequest) -> Result<(), Self::Error>;
}

/// One immediately available observation of the latest retained value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentObservation<'a, T> {
    pub generation: u64,
    pub value: &'a T,
    /// True when the caller's last generation predates more than one
    /// replacement. No displaced values are retained or replayed.
    pub skipped_replacements: bool,
}

/// Replacement failure. The cell remains unchanged for every variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CurrentValueUpdateError<E> {
    GenerationExhausted,
    Unauthorized(E),
}

/// Invalid cursor supplied by an observer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentObservationError {
    FutureGeneration,
}

/// One fixed-resident current value and monotonic replacement generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentValueCell<T> {
    generation: u64,
    value: T,
}

impl<T> CurrentValueCell<T> {
    /// Creates an immediately observable current value at generation zero.
    #[must_use]
    pub const fn new(initial: T) -> Self {
        Self {
            generation: 0,
            value: initial,
        }
    }

    /// Returns the current value immediately. Observer presence is not stored.
    #[must_use]
    pub const fn observe(&self) -> CurrentObservation<'_, T> {
        CurrentObservation {
            generation: self.generation,
            value: &self.value,
            skipped_replacements: false,
        }
    }

    /// Returns the newest current value when it differs from the caller's
    /// generation. Intermediate replacements are intentionally not history.
    pub fn observe_since(
        &self,
        generation: u64,
    ) -> Result<Option<CurrentObservation<'_, T>>, CurrentObservationError> {
        if generation > self.generation {
            return Err(CurrentObservationError::FutureGeneration);
        }
        if generation == self.generation {
            return Ok(None);
        }
        Ok(Some(CurrentObservation {
            generation: self.generation,
            value: &self.value,
            skipped_replacements: self.generation - generation > 1,
        }))
    }

    /// Replaces the current value only after the separate authority boundary
    /// admits the exact generation transition.
    ///
    /// The returned value is the displaced current value, not retained
    /// history. Equal values still count as replacements; this layer performs
    /// no implicit equality deduplication.
    pub fn replace<A: CurrentValueMutationAuthorizer>(
        &mut self,
        next: T,
        authorizer: &mut A,
    ) -> Result<T, CurrentValueUpdateError<A::Error>> {
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(CurrentValueUpdateError::GenerationExhausted)?;
        authorizer
            .authorize(CurrentUpdateRequest {
                current_generation: self.generation,
                next_generation,
            })
            .map_err(CurrentValueUpdateError::Unauthorized)?;
        let previous = core::mem::replace(&mut self.value, next);
        self.generation = next_generation;
        Ok(previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Authorizer {
        admit: bool,
        requests: usize,
    }

    impl CurrentValueMutationAuthorizer for Authorizer {
        type Error = &'static str;

        fn authorize(&mut self, request: CurrentUpdateRequest) -> Result<(), Self::Error> {
            self.requests += 1;
            assert_eq!(request.next_generation, request.current_generation + 1);
            if self.admit { Ok(()) } else { Err("denied") }
        }
    }

    #[test]
    fn initial_and_late_observers_receive_the_current_value_immediately() {
        let mut cell = CurrentValueCell::new("initial");
        assert_eq!(cell.observe().value, &"initial");

        let mut authorizer = Authorizer {
            admit: true,
            requests: 0,
        };
        cell.replace("unobserved-update", &mut authorizer).unwrap();

        let late = cell.observe();
        assert_eq!(late.generation, 1);
        assert_eq!(late.value, &"unobserved-update");
        assert!(!late.skipped_replacements);
    }

    #[test]
    fn reconnect_receives_only_the_newest_value_and_reports_a_history_gap() {
        let mut cell = CurrentValueCell::new(1_u8);
        let disconnected_at = cell.observe().generation;
        let mut authorizer = Authorizer {
            admit: true,
            requests: 0,
        };
        cell.replace(2, &mut authorizer).unwrap();
        cell.replace(3, &mut authorizer).unwrap();

        let current = cell.observe_since(disconnected_at).unwrap().unwrap();
        assert_eq!(current.value, &3);
        assert!(current.skipped_replacements);
        assert_eq!(cell.observe_since(current.generation), Ok(None));
    }

    #[test]
    fn denied_and_exhausted_replacements_leave_state_unchanged() {
        let mut cell = CurrentValueCell::new(1_u8);
        let mut authorizer = Authorizer::default();
        assert_eq!(
            cell.replace(2, &mut authorizer),
            Err(CurrentValueUpdateError::Unauthorized("denied"))
        );
        assert_eq!(cell.observe().value, &1);

        cell.generation = u64::MAX;
        authorizer.admit = true;
        assert_eq!(
            cell.replace(3, &mut authorizer),
            Err(CurrentValueUpdateError::GenerationExhausted)
        );
        assert_eq!(cell.observe().value, &1);
        assert_eq!(authorizer.requests, 1);
    }

    #[test]
    fn equal_replacement_is_not_implicitly_deduplicated() {
        let mut cell = CurrentValueCell::new(7_u8);
        let mut authorizer = Authorizer {
            admit: true,
            requests: 0,
        };
        assert_eq!(cell.replace(7, &mut authorizer), Ok(7));
        assert_eq!(cell.observe().generation, 1);
    }
}
