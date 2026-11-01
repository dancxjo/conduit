//! Quiescent, exact std software expression selection on the retained Host.
//! The descriptor preparation estimate is an explicit operation quote, not an
//! allocator admission, whole-Root workspace proof, authority, or grant.
use crate::StdHost;
use conduit_core as c;
use conduit_plot::PortableExpressionProgram;

/// Selected finite std profile: at most sixteen exact programs at a time.
/// This bounds descriptor/instance staging independently of hosted execution.
pub const MAXIMUM_SELECTED_EXPRESSION_PROGRAMS: usize = 16;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionProgramSelectionEvidence {
    pub offer_generation: c::OfferGeneration,
    pub programs: usize,
    pub descriptor_preparation_estimate_bytes: u64,
}
impl StdHost {
    /// Replace only this Host's selected software expression Base. Empty input
    /// retires it. Programs are source-bound canonical descriptions; callers
    /// cannot inject arbitrary offers. Old Plans retain their old generation.
    pub fn select_expression_programs(
        &mut self,
        programs: &[(PortableExpressionProgram, c::PortTemporal)],
        preparation_limit_bytes: u64,
    ) -> Result<ExpressionProgramSelectionEvidence, String> {
        if programs.len() > MAXIMUM_SELECTED_EXPRESSION_PROGRAMS {
            return Err("selected expression profile exceeds sixteen programs".into());
        }
        let generation = c::OfferGeneration(
            self.advertisement
                .offer_generation
                .0
                .checked_add(1)
                .ok_or("expression offer generation exhausted")?,
        );
        let old = match self.expression_program_base.as_ref() {
            Some(instance) => Some(
                self.base_registry
                    .entries()
                    .iter()
                    .find(|entry| &entry.provider_instance_id == instance)
                    .ok_or("selected expression provider is absent")?,
            ),
            None => None,
        };
        if programs.is_empty() && old.is_none() {
            return Err("no selected expression Base to retire".into());
        }
        // Shared counting encoder checks canonical limits without allocating an
        // encoded program. At minimum the factory retains its hex descriptor.
        let canonical =
            programs
                .iter()
                .try_fold(0u64, |sum, (program, _)| -> Result<u64, String> {
                    let bytes = program
                        .canonical_byte_len()
                        .map_err(|e| format!("expression program: {e:?}"))?
                        as u64;
                    sum.checked_add(
                        bytes
                            .checked_mul(2)
                            .ok_or("expression descriptor overflow")?,
                    )
                    .ok_or_else(|| "expression descriptor overflow".to_owned())
                })?;
        if canonical > preparation_limit_bytes || preparation_limit_bytes == 0 {
            return Err("expression descriptor exceeds selected preparation estimate limit".into());
        }
        if let Some(prior) = old {
            if !self
                .base_registry
                .entries()
                .iter()
                .any(|entry| entry == prior)
                || !prior.capabilities.iter().all(|offer| {
                    self.advertisement
                        .capabilities
                        .iter()
                        .filter(|current| *current == offer)
                        .count()
                        == 1
                })
                || (prior.lifecycle == c::BaseLifecycle::Ready)
                    != self.advertisement.bases.iter().any(|entry| {
                        entry.base_id == prior.base_id
                            && entry.provider_instance_id == prior.provider_instance_id
                            && entry.provider_generation == prior.provider_generation
                    })
            {
                return Err("selected expression inventory differs from current Host".into());
            }
        }
        let mut capabilities = Vec::with_capacity(programs.len());
        for (program, temporal) in programs {
            // Refuse unsupported/invalid evaluator programs rather than merely
            // advertising a syntactically encodable semantic description.
            conduit_plot::PreparedPortableExpressionEvaluator::new(program)
                .map_err(|e| format!("unsupported expression program: {e:?}"))?;
            let offer = conduit_std_offers::pure_expression_std_offer(program, *temporal)
                .map_err(|e| format!("expression factory: {e:?}"))?;
            if !offer.resource_requirements.is_empty() || !offer.authority_requirements.is_empty() {
                return Err("expression factory cannot add resources or authority".into());
            }
            if capabilities
                .iter()
                .any(|prior: &c::CapabilityOffer| prior.capability_id == offer.capability_id)
            {
                return Err("duplicate selected expression program".into());
            }
            capabilities.push(offer);
        }
        let base = c::BaseProviderEntry {
            base_id: "std/selected-expression-programs".into(),
            provider_instance_id: "std/selected-expression-programs/owner".into(),
            provider_generation: generation.0,
            implementation_id: "std/selected-expression-programs@1".into(),
            mechanism_family: "software/pure-expression".into(),
            enforcement_class: c::BaseEnforcementClass::Cooperative,
            lifecycle: if programs.is_empty() {
                c::BaseLifecycle::Stopped
            } else {
                c::BaseLifecycle::Ready
            },
            capabilities,
            resources: Vec::new(),
        };
        let provider = c::BaseProviderAdvertisement {
            base_id: base.base_id.clone(),
            provider_instance_id: base.provider_instance_id.clone(),
            provider_generation: base.provider_generation,
            implementation_id: base.implementation_id.clone(),
            mechanism_family: base.mechanism_family.clone(),
            enforcement_class: base.enforcement_class,
            lifecycle: base.lifecycle,
            capability_ids: base
                .capabilities
                .iter()
                .map(|offer| offer.capability_id.clone())
                .collect(),
            resource_pool_ids: Vec::new(),
        };
        let ready = base.lifecycle == c::BaseLifecycle::Ready;
        let new_capabilities = base.capabilities.clone();
        let backing = self
            .native_source
            .as_ref()
            .and_then(|source| source.backing.as_ref());
        let ledger = self.kernel_resources.prepare_scoped_base_refresh(
            &self.advertisement,
            old,
            &base,
            backing,
        )?;
        let before_caps = self.advertisement.capabilities.capacity();
        let before_bases = self.advertisement.bases.capacity();
        self.advertisement
            .capabilities
            .try_reserve_exact(
                new_capabilities
                    .len()
                    .saturating_sub(old.map_or(0, |prior| prior.capabilities.len())),
            )
            .map_err(|_| "expression capability vector unavailable")?;
        self.advertisement
            .bases
            .try_reserve_exact(usize::from(
                ready && old.is_none_or(|prior| prior.lifecycle != c::BaseLifecycle::Ready),
            ))
            .map_err(|_| "expression Base vector unavailable")?;
        let previous = old.map(|prior| {
            (
                prior.provider_instance_id.clone(),
                prior.provider_generation,
            )
        });
        let old_ids: Vec<_> = old
            .into_iter()
            .flat_map(|prior| &prior.capabilities)
            .map(|offer| offer.capability_id.clone())
            .collect();
        let selected_instance = base.provider_instance_id.clone();
        let selected_base_id = base.base_id.clone();
        let identity_workspace = old_ids.capacity() * std::mem::size_of::<c::CapabilityId>()
            + old_ids.iter().map(|id| id.as_str().len()).sum::<usize>()
            + previous.as_ref().map_or(0, |(id, _)| id.as_str().len())
            + selected_instance.as_str().len()
            + selected_base_id.as_str().len();
        let estimate = preparation::estimate(&base, &provider, canonical)?
            .checked_add(ledger.workspace_bytes)
            .and_then(|n| n.checked_add(identity_workspace as u64))
            .and_then(|n| {
                n.checked_add(
                    growth(
                        before_caps,
                        self.advertisement.capabilities.capacity(),
                        std::mem::size_of::<c::CapabilityOffer>(),
                    )
                    .ok()?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    growth(
                        before_bases,
                        self.advertisement.bases.capacity(),
                        std::mem::size_of::<c::BaseProviderAdvertisement>(),
                    )
                    .ok()?,
                )
            })
            .and_then(|n| {
                n.checked_add(if old.is_none() {
                    (self.base_registry.entries().len() * 2 + 1)
                        .checked_mul(std::mem::size_of::<c::BaseProviderEntry>())?
                        as u64
                } else {
                    0
                })
            })
            .ok_or("expression preparation estimate overflow")?;
        if estimate > preparation_limit_bytes {
            return Err(format!("expression descriptor preparation estimate requires {estimate} bytes, selected {preparation_limit_bytes}"));
        }
        let registry = self
            .base_registry
            .prepare_update(
                previous
                    .as_ref()
                    .map(|(instance, generation)| (instance, *generation)),
                base,
            )
            .map_err(|e| format!("expression Base selection: {e:?}"))?;
        let prior = self.advertisement.offer_generation;
        let boot = &self.advertisement.boot_id;
        if self
            .playback
            .as_ref()
            .is_some_and(|p| p.boot_id != *boot || p.offer_generation != prior)
            || self
                .wav_artifact
                .as_ref()
                .is_some_and(|p| p.boot_id != *boot || p.offer_generation != prior)
        {
            return Err("retained spoken resource differs from current Host offer".into());
        }
        if let Some(adapter) = &mut self.speech_synthesis {
            adapter
                .advance_offer_generation(&self.advertisement.host_id, boot, prior, generation)
                .map_err(|e| format!("retained speech provider: {e:?}"))?;
        }
        // All refusals precede the infallible inventory/accounting commit.
        if let Some(p) = &mut self.playback {
            p.offer_generation = generation;
        }
        if let Some(p) = &mut self.wav_artifact {
            p.offer_generation = generation;
        }
        self.advertisement
            .capabilities
            .retain(|offer| !old_ids.contains(&offer.capability_id));
        self.advertisement
            .bases
            .retain(|entry| entry.base_id != selected_base_id);
        self.advertisement.capabilities.extend(new_capabilities);
        self.advertisement
            .capabilities
            .sort_unstable_by(|a, b| a.capability_id.cmp(&b.capability_id));
        if ready {
            self.advertisement.bases.push(provider);
        }
        self.advertisement.offer_generation = generation;
        registry.commit();
        self.kernel_resources.apply_scoped_base_refresh(ledger);
        if let Some(backing) = self
            .native_source
            .as_mut()
            .and_then(|source| source.backing.as_mut())
        {
            backing.adopt_refreshed_ledger(&self.kernel_resources);
        }
        self.expression_program_base = Some(selected_instance);
        Ok(ExpressionProgramSelectionEvidence {
            offer_generation: generation,
            programs: programs.len(),
            descriptor_preparation_estimate_bytes: estimate,
        })
    }
}
fn growth(before: usize, after: usize, item: usize) -> Result<u64, String> {
    if after <= before {
        return Ok(0);
    }
    before
        .checked_add(after)
        .and_then(|n| n.checked_mul(item))
        .map(|n| n as u64)
        .ok_or("expression vector growth overflow".into())
}
mod preparation;
#[cfg(test)]
mod tests;
