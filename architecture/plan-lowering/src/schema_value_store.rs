//! Caller-selected finite value representation, not a semantic or resource grant.
//! Schema bytes are borrowed (and may be linked flash); only nodes are retained.
use alloc::vec::Vec;
use conduit_core::{
    BorrowedStructuredValueValidator as Validator, ValidatedCanonicalStructuredValue,
};
use conduit_kernel::{CanonicalValueParts, StorageError as E, ValueRef, ValueStorage};

#[derive(Debug, PartialEq, Eq)]
pub enum PreparationError {
    Storage(E),
    Capacity { required: usize, available: usize },
    Allocation,
}
struct Slot<'a> {
    validator: Validator<'a>,
    generation: u16,
    references: u16,
    node: Vec<u8>,
}
/// Optional typed pool alongside an ordinary canonical store. Logical lengths
/// and quotas include schema bytes. Foreign schemas, malformed nodes and stale
/// references refuse; plain `get` never presents a value node as canonical data.
pub struct SchemaValueStore<'a, S> {
    canonical: S,
    canonical_slots: u16,
    slots: Vec<Slot<'a>>,
    byte_capacity: u32,
    used_items: u16,
    used_bytes: u32,
}
impl<'a, S: ValueStorage> SchemaValueStore<'a, S> {
    /// Fund every node slot before Play. The backing ceiling covers this pool's
    /// requested buffers and slot metadata; canonical storage, inline Self,
    /// borrowed schema custody and allocator overhead are separately funded.
    pub fn try_new(
        canonical: S,
        definitions: &[(Validator<'a>, u16)],
        byte_capacity: u32,
        maximum_backing: usize,
    ) -> Result<Self, PreparationError> {
        use PreparationError as P;
        let mut count = 0usize;
        let mut logical = 0u32;
        let mut payload = 0usize;
        for (validator, n) in definitions {
            if *n == 0 {
                return Err(P::Storage(E::InvalidBudget));
            }
            count = count
                .checked_add(usize::from(*n))
                .ok_or(P::Storage(E::InvalidBudget))?;
            let maximum = u32::try_from(validator.maximum_bytes())
                .map_err(|_| P::Storage(E::InvalidBudget))?;
            logical = maximum
                .checked_mul(u32::from(*n))
                .and_then(|n| logical.checked_add(n))
                .ok_or(P::Storage(E::InvalidBudget))?;
            payload = (validator.maximum_bytes() - validator.type_bytes().len())
                .checked_mul(usize::from(*n))
                .and_then(|n| payload.checked_add(n))
                .ok_or(P::Storage(E::InvalidBudget))?;
        }
        let slots_count = u16::try_from(count).map_err(|_| P::Storage(E::InvalidBudget))?;
        let canonical_slots = canonical.item_capacity();
        if count == 0
            || byte_capacity == 0
            || byte_capacity > logical
            || canonical_slots.checked_add(slots_count).is_none()
            || canonical
                .byte_capacity()
                .checked_add(byte_capacity)
                .is_none()
        {
            return Err(P::Storage(E::InvalidBudget));
        }
        let required = count
            .checked_mul(core::mem::size_of::<Slot<'a>>())
            .and_then(|n| n.checked_add(payload))
            .ok_or(P::Storage(E::InvalidBudget))?;
        if required > maximum_backing {
            return Err(P::Capacity {
                required,
                available: maximum_backing,
            });
        }
        let mut slots = Vec::new();
        slots.try_reserve_exact(count).map_err(|_| P::Allocation)?;
        let mut actual = slots
            .capacity()
            .checked_mul(core::mem::size_of::<Slot<'a>>())
            .ok_or(P::Storage(E::InvalidBudget))?;
        for (validator, n) in definitions {
            for _ in 0..*n {
                let mut node = Vec::new();
                node.try_reserve_exact(validator.maximum_bytes() - validator.type_bytes().len())
                    .map_err(|_| P::Allocation)?;
                actual = actual
                    .checked_add(node.capacity())
                    .ok_or(P::Storage(E::InvalidBudget))?;
                if actual > maximum_backing {
                    return Err(P::Capacity {
                        required: actual,
                        available: maximum_backing,
                    });
                }
                slots.push(Slot {
                    validator: *validator,
                    generation: 0,
                    references: 0,
                    node,
                });
            }
        }
        Ok(Self {
            canonical,
            canonical_slots,
            slots,
            byte_capacity,
            used_items: 0,
            used_bytes: 0,
        })
    }
    /// Borrow the original canonical owner for physical custody accounting.
    /// This neither transfers storage nor changes references or availability.
    pub fn canonical_storage(&self) -> &S {
        &self.canonical
    }
    pub fn reserved_node_storage_bytes(&self) -> usize {
        self.slots.capacity() * core::mem::size_of::<Slot<'a>>()
            + self.slots.iter().map(|s| s.node.capacity()).sum::<usize>()
    }
    fn index(&self, value: ValueRef) -> Option<usize> {
        value
            .slot
            .checked_sub(self.canonical_slots)
            .map(usize::from)
    }
    fn slot(&self, value: ValueRef) -> Result<&Slot<'a>, E> {
        let slot = self
            .index(value)
            .and_then(|i| self.slots.get(i))
            .ok_or(E::StaleReference)?;
        if slot.references == 0
            || slot.generation != value.generation
            || slot
                .validator
                .type_bytes()
                .len()
                .checked_add(slot.node.len())
                .and_then(|n| u32::try_from(n).ok())
                != Some(value.byte_len)
        {
            return Err(E::StaleReference);
        }
        Ok(slot)
    }
    /// Publish only an already checked value under the exact installed schema.
    /// Selection, live quotas and all capacity checks precede mutation.
    pub fn store_structured(
        &mut self,
        value: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<ValueRef, E> {
        let len = value
            .type_bytes()
            .len()
            .checked_add(value.value_node().len())
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(E::ValueTooLarge)?;
        let used = self
            .used_bytes
            .checked_add(len)
            .filter(|n| *n <= self.byte_capacity)
            .ok_or(E::ByteCapacityExceeded)?;
        let mut retired = false;
        let mut known = false;
        let index = self
            .slots
            .iter()
            .position(|slot| {
                let fits = slot.validator.type_bytes() == value.type_bytes()
                    && usize::try_from(len).is_ok_and(|n| n <= slot.validator.maximum_bytes());
                known |= fits;
                retired |= fits && slot.references == 0 && slot.generation == u16::MAX;
                fits && slot.references == 0 && slot.generation != u16::MAX
            })
            .ok_or(if retired {
                E::GenerationExhausted
            } else if known {
                E::ItemCapacityExceeded
            } else {
                E::RepresentationMismatch
            })?;
        let slot = &mut self.slots[index];
        // ValidatedCanonicalStructuredValue is immutable. Recheck against the
        // installed ceiling before copying; no unchecked Type witness is made.
        slot.validator
            .validate_node(value.value_node())
            .map_err(|_| E::RepresentationMismatch)?;
        if value.value_node().len() > slot.node.capacity() {
            return Err(E::ValueTooLarge);
        }
        slot.generation += 1;
        slot.references = 1;
        slot.node.clear();
        slot.node.extend_from_slice(value.value_node());
        self.used_items += 1;
        self.used_bytes = used;
        Ok(ValueRef {
            slot: self.canonical_slots + index as u16,
            generation: slot.generation,
            byte_len: len,
        })
    }
}
impl<S: ValueStorage> ValueStorage for SchemaValueStore<'_, S> {
    fn item_capacity(&self) -> u16 {
        self.canonical_slots + self.slots.len() as u16
    }
    fn byte_capacity(&self) -> u32 {
        self.canonical.byte_capacity() + self.byte_capacity
    }
    fn used_items(&self) -> u16 {
        self.canonical.used_items() + self.used_items
    }
    fn used_bytes(&self) -> u32 {
        self.canonical.used_bytes() + self.used_bytes
    }
    fn preflight_canonical_store(&self, len: u32) -> Result<(), E> {
        self.canonical.preflight_canonical_store(len)
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, E> {
        self.canonical.store(bytes)
    }
    fn supports_schema_storage(&self, schema: &[u8]) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.validator.type_bytes() == schema)
    }
    fn store_schema_canonical(&mut self, schema: &[u8], bytes: &[u8]) -> Result<ValueRef, E> {
        let validator = self
            .slots
            .iter()
            .find(|slot| slot.validator.type_bytes() == schema)
            .map(|slot| slot.validator)
            .ok_or(E::RepresentationMismatch)?;
        let checked = validator
            .validate(bytes)
            .map_err(|_| E::RepresentationMismatch)?;
        if checked.type_bytes() != schema {
            return Err(E::RepresentationMismatch);
        }
        self.store_structured(checked)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], E> {
        if self.index(value).is_some() {
            self.slot(value)?;
            Err(E::RepresentationMismatch)
        } else {
            self.canonical.get(value)
        }
    }
    fn canonical_parts(&self, value: ValueRef) -> Result<Option<CanonicalValueParts<'_>>, E> {
        if self.index(value).is_none() {
            return self.canonical.canonical_parts(value);
        }
        let slot = self.slot(value)?;
        Ok(Some(CanonicalValueParts {
            schema: slot.validator.type_bytes(),
            node: &slot.node,
        }))
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, E> {
        if self.index(value).is_some() {
            Ok(self.slot(value)?.references)
        } else {
            self.canonical.reference_count(value)
        }
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), E> {
        if let Some(i) = self.index(value) {
            let n = self
                .slot(value)?
                .references
                .checked_add(1)
                .ok_or(E::ReferenceOverflow)?;
            self.slots[i].references = n;
            Ok(())
        } else {
            self.canonical.retain(value)
        }
    }
    fn release(&mut self, value: ValueRef) -> Result<(), E> {
        if let Some(i) = self.index(value) {
            self.slot(value)?;
            self.slots[i].references -= 1;
            if self.slots[i].references == 0 {
                self.used_items -= 1;
                self.used_bytes -= value.byte_len;
                self.slots[i].node.clear();
            }
            Ok(())
        } else {
            self.canonical.release(value)
        }
    }
    fn clear(&mut self) {
        self.canonical.clear();
        for slot in &mut self.slots {
            slot.references = 0;
            slot.node.clear();
        }
        self.used_items = 0;
        self.used_bytes = 0;
    }
}
#[cfg(test)]
mod tests;
