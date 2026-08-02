//! Bounded hosted implementations for the standard temporal conversions.

use crate::{CurrentObservation, CurrentObservationError, CurrentValueCell};

/// One item or the explicit normal close of a closing flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClosingFlowEvent<T> {
    Item(T),
    Closed,
}

/// One item of an open flow. This type has no normal-close variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenFlowItem<T>(pub T);

/// `flow/each`: one already-bounded list becomes an ordered closing flow.
#[derive(Clone, Debug)]
pub struct EachClosingFlow<T> {
    items: std::vec::IntoIter<T>,
    close_emitted: bool,
}

impl<T> EachClosingFlow<T> {
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items: items.into_iter(),
            close_emitted: false,
        }
    }

    /// Emits every item in list order, then exactly one normal close.
    pub fn next_event(&mut self) -> Option<ClosingFlowEvent<T>> {
        if let Some(item) = self.items.next() {
            return Some(ClosingFlowEvent::Item(item));
        }
        if self.close_emitted {
            return None;
        }
        self.close_emitted = true;
        Some(ClosingFlowEvent::Closed)
    }
}

/// Finite admission bounds for `flow/collect`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollectLimits {
    pub maximum_items: usize,
    pub maximum_bytes: u64,
}

impl CollectLimits {
    pub fn validate(self) -> Result<(), CollectError> {
        if self.maximum_items == 0 || self.maximum_bytes == 0 {
            return Err(CollectError::InvalidLimits);
        }
        Ok(())
    }
}

/// Stable `flow/collect` failure reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectError {
    InvalidLimits,
    AllocationUnavailable,
    ItemLimitExceeded,
    ByteLimitExceeded,
    ByteCountOverflow,
    AlreadyClosed,
    NormalCloseRequired,
}

/// Rejected event returned without changing collector state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectRejection<T> {
    pub event: ClosingFlowEvent<T>,
    pub reason: CollectError,
}

/// `flow/collect`: a closing flow accumulated under exact item and byte bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedClosingCollector<T> {
    limits: CollectLimits,
    items: Vec<T>,
    accepted_bytes: u64,
    closed: bool,
}

impl<T> BoundedClosingCollector<T> {
    /// Reserves the complete item-slot bound before accepting flow values.
    pub fn new(limits: CollectLimits) -> Result<Self, CollectError> {
        limits.validate()?;
        let mut items = Vec::new();
        items
            .try_reserve_exact(limits.maximum_items)
            .map_err(|_| CollectError::AllocationUnavailable)?;
        Ok(Self {
            limits,
            items,
            accepted_bytes: 0,
            closed: false,
        })
    }

    /// Accepts one closing-flow event. `item_bytes` is required for an item
    /// and ignored for the zero-byte closing boundary.
    pub fn accept(
        &mut self,
        event: ClosingFlowEvent<T>,
        item_bytes: u64,
    ) -> Result<(), CollectRejection<T>> {
        if self.closed {
            return Err(CollectRejection {
                event,
                reason: CollectError::AlreadyClosed,
            });
        }
        match event {
            ClosingFlowEvent::Closed => {
                self.closed = true;
                Ok(())
            }
            ClosingFlowEvent::Item(item) => {
                if self.items.len() >= self.limits.maximum_items {
                    return Err(CollectRejection {
                        event: ClosingFlowEvent::Item(item),
                        reason: CollectError::ItemLimitExceeded,
                    });
                }
                let Some(next_bytes) = self.accepted_bytes.checked_add(item_bytes) else {
                    return Err(CollectRejection {
                        event: ClosingFlowEvent::Item(item),
                        reason: CollectError::ByteCountOverflow,
                    });
                };
                if next_bytes > self.limits.maximum_bytes {
                    return Err(CollectRejection {
                        event: ClosingFlowEvent::Item(item),
                        reason: CollectError::ByteLimitExceeded,
                    });
                }
                self.items.push(item);
                self.accepted_bytes = next_bytes;
                Ok(())
            }
        }
    }

    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    #[must_use]
    pub fn accepted_items(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub const fn accepted_bytes(&self) -> u64 {
        self.accepted_bytes
    }

    /// Produces one list only after the explicit normal closing boundary.
    pub fn into_list(self) -> Result<Vec<T>, CollectError> {
        if !self.closed {
            return Err(CollectError::NormalCloseRequired);
        }
        Ok(self.items)
    }
}

/// `state/sample`: one ordinary observation of the immediately current value.
#[must_use]
pub const fn sample_current<T>(cell: &CurrentValueCell<T>) -> CurrentObservation<'_, T> {
    cell.observe()
}

/// `state/changes`: an open-flow cursor over future current replacements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentChanges {
    generation: u64,
}

impl CurrentChanges {
    /// Starts after the currently observable value; only later replacements
    /// are emitted as changes.
    #[must_use]
    pub const fn new<T>(cell: &CurrentValueCell<T>) -> Self {
        Self {
            generation: cell.observe().generation,
        }
    }

    /// Returns the newest replacement since the last poll. There is no normal
    /// closing event and no displaced-value history.
    pub fn poll<'a, T>(
        &mut self,
        cell: &'a CurrentValueCell<T>,
    ) -> Result<Option<CurrentObservation<'a, T>>, CurrentObservationError> {
        let observation = cell.observe_since(self.generation)?;
        if let Some(ref current) = observation {
            self.generation = current.generation;
        }
        Ok(observation)
    }
}

/// `state/hold`: an explicit initial value creates the current observation.
#[must_use]
pub const fn hold_current<T>(initial: T) -> CurrentValueCell<T> {
    CurrentValueCell::new(initial)
}
