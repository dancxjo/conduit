//! Full-scan ownership oracle independent of cached free-slot bounds.
use conduit_kernel::{FixedValueStore, StorageError, ValueRef, ValueStorage};
#[derive(Clone, Copy, Default)]
struct Slot {
    generation: u16,
    references: u16,
    len: u32,
    byte: u8,
}
struct Model {
    slots: [Slot; 8],
    capacities: [u32; 8],
}
impl Model {
    fn used_bytes(&self) -> u32 {
        self.slots
            .iter()
            .filter(|s| s.references > 0)
            .map(|s| s.len)
            .sum()
    }
    fn valid(&self, value: ValueRef) -> Result<usize, StorageError> {
        let index = usize::from(value.slot);
        let slot = self.slots.get(index).ok_or(StorageError::StaleReference)?;
        if slot.references == 0 || slot.generation != value.generation || slot.len != value.byte_len
        {
            return Err(StorageError::StaleReference);
        }
        Ok(index)
    }
    fn store(&mut self, len: u32, byte: u8) -> Result<ValueRef, StorageError> {
        if len > *self.capacities.iter().max().unwrap() {
            return Err(StorageError::ValueTooLarge);
        }
        if self.used_bytes() + len > 7 {
            return Err(StorageError::ByteCapacityExceeded);
        }
        let index = (0..8)
            .filter(|&i| {
                self.slots[i].references == 0
                    && self.slots[i].generation < u16::MAX
                    && self.capacities[i] >= len
            })
            .min_by_key(|&i| (self.capacities[i], i))
            .ok_or_else(|| {
                if (0..8).any(|i| {
                    self.slots[i].references == 0
                        && self.slots[i].generation == u16::MAX
                        && self.capacities[i] >= len
                }) {
                    StorageError::GenerationExhausted
                } else {
                    StorageError::ItemCapacityExceeded
                }
            })?;
        let slot = &mut self.slots[index];
        slot.generation += 1;
        slot.references = 1;
        slot.len = len;
        slot.byte = byte;
        Ok(ValueRef {
            slot: index as u16,
            generation: slot.generation,
            byte_len: len,
        })
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), StorageError> {
        let i = self.valid(value)?;
        self.slots[i].references = self.slots[i]
            .references
            .checked_add(1)
            .ok_or(StorageError::ReferenceOverflow)?;
        Ok(())
    }
    fn release(&mut self, value: ValueRef) -> Result<(), StorageError> {
        let i = self.valid(value)?;
        self.slots[i].references -= 1;
        if self.slots[i].references == 0 {
            self.slots[i].len = 0;
        }
        Ok(())
    }
    fn clear(&mut self) {
        for slot in &mut self.slots {
            slot.references = 0;
            slot.len = 0;
        }
    }
    fn verify(&self, actual: &impl ValueStorage) {
        assert_eq!(actual.used_bytes(), self.used_bytes());
        assert_eq!(
            usize::from(actual.used_items()),
            self.slots.iter().filter(|s| s.references > 0).count()
        );
        for (i, slot) in self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.references > 0)
        {
            let value = ValueRef {
                slot: i as u16,
                generation: slot.generation,
                byte_len: slot.len,
            };
            assert_eq!(actual.reference_count(value), Ok(slot.references));
            assert_eq!(
                actual.get(value),
                Ok([slot.byte; 9][..slot.len as usize].as_ref())
            );
        }
    }
}
fn exercise<S: ValueStorage>(mut make: impl FnMut() -> S, capacities: [u32; 8]) {
    let mut successful_mutations = [0u32; 3];
    for seed in 0u32..256 {
        let mut actual = make();
        let mut model = Model {
            slots: [Slot::default(); 8],
            capacities,
        };
        let mut history = [None; 32];
        let mut next_handle = 0;
        let mut random = seed;
        for _ in 0..256 {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let action = (random >> 24) % 8;
            let value = history[((random >> 8) % 32) as usize].unwrap_or(ValueRef {
                slot: 9,
                generation: 0,
                byte_len: 0,
            });
            match action {
                0..=2 => {
                    let len = (random >> 16) % 10;
                    let byte = random as u8;
                    let expected = model.store(len, byte);
                    let result = actual.store(&[byte; 9][..len as usize]);
                    assert_eq!(result, expected);
                    if let Ok(value) = result {
                        history[next_handle % 32] = Some(value);
                        next_handle += 1;
                        successful_mutations[0] += 1;
                    }
                }
                3 => {
                    let expected = model.retain(value);
                    assert_eq!(actual.retain(value), expected);
                    successful_mutations[1] += u32::from(expected.is_ok());
                }
                4 => {
                    let expected = model.release(value);
                    assert_eq!(actual.release(value), expected);
                    successful_mutations[2] += u32::from(expected.is_ok());
                }
                5 => {
                    model.clear();
                    actual.clear();
                }
                6 => {
                    let expected = model.valid(value).map(|i| &model.slots[i]);
                    assert_eq!(
                        actual
                            .get(value)
                            .map(|bytes| (bytes.len(), bytes.first().copied())),
                        expected.map(|s| (s.len as usize, (s.len > 0).then_some(s.byte)))
                    );
                }
                _ => {
                    assert_eq!(
                        actual.reference_count(value),
                        model.valid(value).map(|i| model.slots[i].references)
                    );
                }
            }
            model.verify(&actual);
        }
    }
    assert!(successful_mutations.iter().all(|count| *count > 0));
}
#[test]
fn fixed_store_matches_full_scan_model_through_interleaved_lifecycles() {
    exercise(|| FixedValueStore::<8, 4>::new(7).unwrap(), [4; 8]);
}
#[cfg(feature = "alloc")]
#[test]
fn hosted_uniform_and_mixed_stores_match_full_scan_lifecycle_model() {
    use conduit_kernel::HostedValueStore;
    for capacities in [[4; 8], [8, 1, 4, 2, 8, 1, 2, 4]] {
        exercise(
            || HostedValueStore::new_with_slot_capacities(&capacities, 7).unwrap(),
            capacities,
        );
    }
}
