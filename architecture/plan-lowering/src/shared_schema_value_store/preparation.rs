use super::*;
#[derive(Debug, PartialEq, Eq)]
pub enum PreparationError {
    Storage(E),
    Capacity { required: usize, available: usize },
    Allocation,
}
impl<'a, S: ValueStorage> SharedSchemaValueStore<'a, S> {
    /// Explicit physical stock only: this constructor proves representation,
    /// widths and backing, NOT a graph's maximum simultaneous live values.
    /// Logical declarations preserve the original identities and byte quota.
    pub fn try_new(
        canonical: S,
        logical: &[(Validator<'a>, u16)],
        physical: &[(Validator<'a>, u16)],
        byte_capacity: u32,
        maximum_backing: usize,
    ) -> Result<Self, PreparationError> {
        use PreparationError as P;
        let bad = || P::Storage(E::InvalidBudget);
        let count = |definitions: &[(Validator<'a>, u16)]| {
            definitions.iter().try_fold(0usize, |sum, (_, n)| {
                if *n == 0 {
                    return None;
                }
                sum.checked_add(usize::from(*n))
            })
        };
        let logical_count = count(logical).ok_or_else(bad)?;
        let physical_count = count(physical).ok_or_else(bad)?;
        let canonical_slots = canonical.item_capacity();
        let logical_slots = u16::try_from(logical_count).map_err(|_| bad())?;
        u16::try_from(physical_count).map_err(|_| bad())?;
        let logical_bytes = logical
            .iter()
            .try_fold(0u32, |sum, (v, n)| {
                u32::try_from(v.maximum_bytes())
                    .ok()?
                    .checked_mul(u32::from(*n))?
                    .checked_add(sum)
            })
            .ok_or_else(bad)?;
        if logical_count == 0
            || physical_count == 0
            || byte_capacity == 0
            || byte_capacity > logical_bytes
            || canonical_slots.checked_add(logical_slots).is_none()
            || canonical
                .byte_capacity()
                .checked_add(byte_capacity)
                .is_none()
        {
            return Err(bad());
        }
        // No logical schema/extent may have zero usable physical backing.
        // Counts can be smaller explicitly; runtime pressure remains checked.
        for (v, _) in logical {
            if !physical.iter().any(|(p, _)| {
                p.type_bytes() == v.type_bytes() && p.maximum_bytes() >= v.maximum_bytes()
            }) {
                return Err(P::Storage(E::RepresentationMismatch));
            }
        }
        for (p, _) in physical {
            if !logical
                .iter()
                .any(|(v, _)| v.type_bytes() == p.type_bytes())
            {
                return Err(P::Storage(E::RepresentationMismatch));
            }
        }
        let class_count = logical
            .iter()
            .enumerate()
            .filter(|(i, (v, _))| {
                !logical[..*i]
                    .iter()
                    .any(|(previous, _)| previous.type_bytes() == v.type_bytes())
            })
            .count();
        let metadata = logical_count
            .checked_mul(core::mem::size_of::<LogicalSlot>())
            .and_then(|n| {
                physical_count
                    .checked_mul(core::mem::size_of::<PhysicalBuffer>())
                    .and_then(|p| n.checked_add(p))
            })
            .and_then(|n| {
                class_count
                    .checked_mul(core::mem::size_of::<Validator<'a>>())
                    .and_then(|p| n.checked_add(p))
            })
            .ok_or_else(bad)?;
        let payload = physical
            .iter()
            .try_fold(0usize, |sum, (v, n)| {
                v.maximum_bytes()
                    .checked_sub(v.type_bytes().len())?
                    .checked_mul(usize::from(*n))?
                    .checked_add(sum)
            })
            .ok_or_else(bad)?;
        let required = metadata.checked_add(payload).ok_or_else(bad)?;
        if required > maximum_backing {
            return Err(P::Capacity {
                required,
                available: maximum_backing,
            });
        }
        let mut classes: Vec<Validator<'a>> = Vec::new();
        let mut slots = Vec::new();
        let mut buffers = Vec::new();
        classes
            .try_reserve_exact(class_count)
            .map_err(|_| P::Allocation)?;
        slots
            .try_reserve_exact(logical_count)
            .map_err(|_| P::Allocation)?;
        buffers
            .try_reserve_exact(physical_count)
            .map_err(|_| P::Allocation)?;
        let mut actual = classes
            .capacity()
            .checked_mul(core::mem::size_of::<Validator<'a>>())
            .and_then(|n| {
                slots
                    .capacity()
                    .checked_mul(core::mem::size_of::<LogicalSlot>())
                    .and_then(|p| n.checked_add(p))
            })
            .and_then(|n| {
                buffers
                    .capacity()
                    .checked_mul(core::mem::size_of::<PhysicalBuffer>())
                    .and_then(|p| n.checked_add(p))
            })
            .ok_or_else(bad)?;
        if actual > maximum_backing {
            return Err(P::Capacity {
                required: actual,
                available: maximum_backing,
            });
        }
        for (v, count) in logical {
            let class = match classes
                .iter()
                .position(|c| c.type_bytes() == v.type_bytes())
            {
                Some(i) => {
                    if classes[i].maximum_bytes() < v.maximum_bytes() {
                        classes[i] = *v;
                    }
                    i
                }
                None => {
                    classes.push(*v);
                    classes.len() - 1
                }
            };
            for _ in 0..*count {
                slots.push(LogicalSlot {
                    maximum_bytes: v.maximum_bytes(),
                    class: class as u16,
                    generation: 0,
                    references: 0,
                    buffer: None,
                });
            }
        }
        for (v, count) in physical {
            let class = classes
                .iter()
                .position(|c| c.type_bytes() == v.type_bytes())
                .ok_or(P::Storage(E::RepresentationMismatch))?;
            for _ in 0..*count {
                let mut node = Vec::new();
                node.try_reserve_exact(v.maximum_bytes() - v.type_bytes().len())
                    .map_err(|_| P::Allocation)?;
                actual = actual.checked_add(node.capacity()).ok_or_else(bad)?;
                if actual > maximum_backing {
                    return Err(P::Capacity {
                        required: actual,
                        available: maximum_backing,
                    });
                }
                buffers.push(PhysicalBuffer {
                    node,
                    class: class as u16,
                    owner: None,
                });
            }
        }
        Ok(Self {
            canonical,
            canonical_slots,
            classes,
            logical: slots,
            physical: buffers,
            byte_capacity,
            used_items: 0,
            used_bytes: 0,
        })
    }
}
