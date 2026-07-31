pub const STATE_MAX_ENTRIES: usize = 16;
pub const STATE_MAX_VALUE_BYTES: u64 = 65_536;

pub type StateIdentity = [u8; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateError {
    EntryBoundExceeded,
    ByteBoundExceeded,
    GenerationOverflow,
}

impl StateError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EntryBoundExceeded => "CND-STA-001",
            Self::ByteBoundExceeded => "CND-STA-002",
            Self::GenerationOverflow => "CND-STA-003",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellState<T: Copy> {
    initial: Option<T>,
    current: Option<T>,
    generation: u64,
}

impl<T: Copy> CellState<T> {
    #[must_use]
    pub const fn new(initial: Option<T>) -> Self {
        Self {
            initial,
            current: initial,
            generation: 0,
        }
    }

    #[must_use]
    pub const fn current(&self) -> Option<T> {
        self.current
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn set(&mut self, value: T) -> Result<T, StateError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(StateError::GenerationOverflow)?;
        self.current = Some(value);
        Ok(value)
    }

    pub fn reset(&mut self) -> Result<Option<T>, StateError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(StateError::GenerationOverflow)?;
        self.current = self.initial;
        Ok(self.current)
    }

    pub fn restart(&mut self) {
        self.current = self.initial;
        self.generation = 0;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeduplicateDecision {
    Unique { evicted: Option<StateIdentity> },
    Duplicate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeduplicateState<const N: usize> {
    identities: [Option<StateIdentity>; N],
    bytes: [u32; N],
    len: usize,
    retained_bytes: u64,
    maximum_entries: usize,
    maximum_bytes: u64,
}

impl<const N: usize> DeduplicateState<N> {
    pub fn new(maximum_entries: usize, maximum_bytes: u64) -> Result<Self, StateError> {
        if maximum_entries == 0 || maximum_entries > N {
            return Err(StateError::EntryBoundExceeded);
        }
        if maximum_bytes == 0 || maximum_bytes > STATE_MAX_VALUE_BYTES {
            return Err(StateError::ByteBoundExceeded);
        }
        Ok(Self {
            identities: [None; N],
            bytes: [0; N],
            len: 0,
            retained_bytes: 0,
            maximum_entries,
            maximum_bytes,
        })
    }

    pub fn admit(
        &mut self,
        identity: StateIdentity,
        accounted_bytes: u32,
    ) -> Result<DeduplicateDecision, StateError> {
        let accounted_bytes_u64 = u64::from(accounted_bytes);
        if accounted_bytes_u64 > self.maximum_bytes {
            return Err(StateError::ByteBoundExceeded);
        }
        if self.identities[..self.len].contains(&Some(identity)) {
            return Ok(DeduplicateDecision::Duplicate);
        }
        let evicted = if self.len == self.maximum_entries
            || self
                .retained_bytes
                .checked_add(accounted_bytes_u64)
                .is_none_or(|bytes| bytes > self.maximum_bytes)
        {
            Some(self.evict_oldest())
        } else {
            None
        };
        while self
            .retained_bytes
            .checked_add(accounted_bytes_u64)
            .is_none_or(|bytes| bytes > self.maximum_bytes)
        {
            if self.len == 0 {
                return Err(StateError::ByteBoundExceeded);
            }
            self.evict_oldest();
        }
        self.identities[self.len] = Some(identity);
        self.bytes[self.len] = accounted_bytes;
        self.len += 1;
        self.retained_bytes += accounted_bytes_u64;
        Ok(DeduplicateDecision::Unique { evicted })
    }

    fn evict_oldest(&mut self) -> StateIdentity {
        let evicted = self.identities[0].expect("non-empty deduplicate state");
        self.retained_bytes -= u64::from(self.bytes[0]);
        self.identities.copy_within(1..self.len, 0);
        self.bytes.copy_within(1..self.len, 0);
        self.len -= 1;
        self.identities[self.len] = None;
        self.bytes[self.len] = 0;
        evicted
    }

    pub fn reset(&mut self) {
        self.identities = [None; N];
        self.bytes = [0; N];
        self.len = 0;
        self.retained_bytes = 0;
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheEntry {
    pub key: StateIdentity,
    pub value_handle: u64,
    pub value_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheInsert {
    Inserted { evicted: Option<StateIdentity> },
    Updated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheState<const N: usize> {
    entries: [Option<CacheEntry>; N],
    len: usize,
    retained_bytes: u64,
    maximum_entries: usize,
    maximum_bytes: u64,
}

impl<const N: usize> CacheState<N> {
    pub fn new(maximum_entries: usize, maximum_bytes: u64) -> Result<Self, StateError> {
        if maximum_entries == 0 || maximum_entries > N {
            return Err(StateError::EntryBoundExceeded);
        }
        if maximum_bytes == 0 || maximum_bytes > STATE_MAX_VALUE_BYTES {
            return Err(StateError::ByteBoundExceeded);
        }
        Ok(Self {
            entries: [None; N],
            len: 0,
            retained_bytes: 0,
            maximum_entries,
            maximum_bytes,
        })
    }

    #[must_use]
    pub fn lookup(&self, key: StateIdentity) -> Option<CacheEntry> {
        self.entries[..self.len]
            .iter()
            .flatten()
            .find(|entry| entry.key == key)
            .copied()
    }

    pub fn insert(&mut self, entry: CacheEntry) -> Result<CacheInsert, StateError> {
        let value_bytes = u64::from(entry.value_bytes);
        if value_bytes > self.maximum_bytes {
            return Err(StateError::ByteBoundExceeded);
        }
        if let Some(index) = self.entries[..self.len]
            .iter()
            .position(|candidate| candidate.is_some_and(|candidate| candidate.key == entry.key))
        {
            let prior = self.entries[index].expect("matched cache entry");
            let retained_bytes = self
                .retained_bytes
                .checked_sub(u64::from(prior.value_bytes))
                .and_then(|bytes| bytes.checked_add(value_bytes))
                .ok_or(StateError::ByteBoundExceeded)?;
            if retained_bytes > self.maximum_bytes {
                return Err(StateError::ByteBoundExceeded);
            }
            self.entries[index] = Some(entry);
            self.retained_bytes = retained_bytes;
            return Ok(CacheInsert::Updated);
        }
        let evicted = if self.len == self.maximum_entries
            || self
                .retained_bytes
                .checked_add(value_bytes)
                .is_none_or(|bytes| bytes > self.maximum_bytes)
        {
            Some(self.evict_oldest().key)
        } else {
            None
        };
        while self
            .retained_bytes
            .checked_add(value_bytes)
            .is_none_or(|bytes| bytes > self.maximum_bytes)
        {
            if self.len == 0 {
                return Err(StateError::ByteBoundExceeded);
            }
            self.evict_oldest();
        }
        self.entries[self.len] = Some(entry);
        self.len += 1;
        self.retained_bytes += value_bytes;
        Ok(CacheInsert::Inserted { evicted })
    }

    pub fn invalidate(&mut self, key: StateIdentity) -> bool {
        let Some(index) = self.entries[..self.len]
            .iter()
            .position(|candidate| candidate.is_some_and(|candidate| candidate.key == key))
        else {
            return false;
        };
        let removed = self.entries[index].expect("matched cache entry");
        self.retained_bytes -= u64::from(removed.value_bytes);
        self.entries.copy_within(index + 1..self.len, index);
        self.len -= 1;
        self.entries[self.len] = None;
        true
    }

    fn evict_oldest(&mut self) -> CacheEntry {
        let evicted = self.entries[0].expect("non-empty cache state");
        self.retained_bytes -= u64::from(evicted.value_bytes);
        self.entries.copy_within(1..self.len, 0);
        self.len -= 1;
        self.entries[self.len] = None;
        evicted
    }

    pub fn restart(&mut self) {
        self.entries = [None; N];
        self.len = 0;
        self.retained_bytes = 0;
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn key(byte: u8) -> StateIdentity {
        [byte; 32]
    }

    #[test]
    fn cell_initial_set_reset_and_restart_are_exact() {
        let mut empty = CellState::<u64>::new(None);
        assert_eq!(empty.current(), None);
        assert_eq!(empty.set(4), Ok(4));
        assert_eq!(empty.current(), Some(4));
        assert_eq!(empty.reset(), Ok(None));

        let mut initialized = CellState::new(Some(2_u64));
        assert_eq!(initialized.set(3), Ok(3));
        assert_eq!(initialized.generation(), 1);
        initialized.restart();
        assert_eq!(initialized.current(), Some(2));
        assert_eq!(initialized.generation(), 0);
    }

    #[test]
    fn deduplicate_is_collision_safe_bounded_and_fifo() {
        let mut state = DeduplicateState::<2>::new(2, 8).unwrap();
        assert_eq!(
            state.admit(key(1), 4),
            Ok(DeduplicateDecision::Unique { evicted: None })
        );
        assert_eq!(state.admit(key(1), 4), Ok(DeduplicateDecision::Duplicate));
        assert_eq!(
            state.admit(key(2), 4),
            Ok(DeduplicateDecision::Unique { evicted: None })
        );
        assert_eq!(
            state.admit(key(3), 4),
            Ok(DeduplicateDecision::Unique {
                evicted: Some(key(1))
            })
        );
        assert_eq!(state.len(), 2);
        assert_eq!(state.retained_bytes(), 8);
        state.reset();
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn cache_hit_miss_update_invalidate_fifo_and_restart_are_exact() {
        let mut state = CacheState::<2>::new(2, 8).unwrap();
        let first = CacheEntry {
            key: key(1),
            value_handle: 10,
            value_bytes: 4,
        };
        assert_eq!(state.lookup(key(1)), None);
        assert_eq!(
            state.insert(first),
            Ok(CacheInsert::Inserted { evicted: None })
        );
        assert_eq!(state.lookup(key(1)), Some(first));
        assert_eq!(
            state.insert(CacheEntry {
                value_handle: 11,
                ..first
            }),
            Ok(CacheInsert::Updated)
        );
        state
            .insert(CacheEntry {
                key: key(2),
                value_handle: 20,
                value_bytes: 4,
            })
            .unwrap();
        assert_eq!(
            state.insert(CacheEntry {
                key: key(3),
                value_handle: 30,
                value_bytes: 4,
            }),
            Ok(CacheInsert::Inserted {
                evicted: Some(key(1))
            })
        );
        assert!(state.invalidate(key(2)));
        assert!(!state.invalidate(key(2)));
        state.restart();
        assert_eq!(state.len(), 0);
    }
}
