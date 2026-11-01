//! Exclusive registry update preparation and infallible commit.
use super::{validate_entry, BaseProviderEntry, BaseRegistry, BaseRegistryRefusal};
use crate::BaseInstanceId;

/// Exclusive preflighted registry update. Construction validates the new
/// descriptor and reserves entry backing; commit moves only that descriptor.
/// Unrelated provider payloads are neither cloned nor reconstructed.
pub struct PreparedBaseProviderUpdate<'a> {
    registry: &'a mut BaseRegistry,
    index: Option<usize>,
    entry: BaseProviderEntry,
}
impl PreparedBaseProviderUpdate<'_> {
    pub fn commit(self) {
        match self.index {
            Some(index) => self.registry.entries[index] = self.entry,
            None => self.registry.entries.push(self.entry),
        }
        self.registry.revision = self.registry.revision.saturating_add(1);
    }
}
impl BaseRegistry {
    /// Preflight a register/replacement without copying existing equipment.
    /// The exclusive guard prevents a different update between check and apply.
    pub fn prepare_update(
        &mut self,
        previous: Option<(&BaseInstanceId, u64)>,
        entry: BaseProviderEntry,
    ) -> Result<PreparedBaseProviderUpdate<'_>, BaseRegistryRefusal> {
        validate_entry(&entry, self.limits)?;
        let index = match previous {
            Some((instance, generation)) => {
                let index = self
                    .entries
                    .iter()
                    .position(|prior| prior.base_id == entry.base_id)
                    .ok_or(BaseRegistryRefusal::UnknownBase)?;
                let prior = &self.entries[index];
                if prior.provider_instance_id != *instance
                    || prior.provider_generation != generation
                {
                    return Err(BaseRegistryRefusal::StaleProvider);
                }
                if entry.provider_generation <= generation {
                    return Err(BaseRegistryRefusal::GenerationDidNotAdvance);
                }
                Some(index)
            }
            None => {
                if self.entries.len() >= self.limits.maximum_bases as usize {
                    return Err(BaseRegistryRefusal::RegistryFull);
                }
                if self
                    .entries
                    .iter()
                    .any(|prior| prior.base_id == entry.base_id)
                {
                    return Err(BaseRegistryRefusal::DuplicateBase);
                }
                None
            }
        };
        if self.entries.iter().enumerate().any(|(i, prior)| {
            Some(i) != index && prior.provider_instance_id == entry.provider_instance_id
        }) {
            return Err(BaseRegistryRefusal::DuplicateProviderInstance);
        }
        if index.is_none() {
            self.entries
                .try_reserve_exact(1)
                .map_err(|_| BaseRegistryRefusal::RegistryFull)?;
        }
        Ok(PreparedBaseProviderUpdate {
            registry: self,
            index,
            entry,
        })
    }
}
