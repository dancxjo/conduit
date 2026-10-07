//! Root-owned exact admission for the ordinary text region's serial effect.
use crate::{
    machine::BaseKind,
    offer::{BaseProviderBinding, HostOffer, TEXT_PRESENTATION_IMPLEMENTATION},
    protected_region::{DomainRefusal, RegionBinding},
    protection_domain::KernelCapabilityScope,
};
use conduit_core::Plan;
use sha2::{Digest, Sha256};

pub const SERIAL_PRESENT_OPERATION: u32 = 7;

pub struct SerialScope {
    pub scope: KernelCapabilityScope,
    pub provider: BaseProviderBinding,
}

impl SerialScope {
    pub fn admit(
        plan: &Plan,
        binding: &RegionBinding,
        fixed: &HostOffer<'_>,
    ) -> Result<Self, DomainRefusal> {
        // Admission runs during preparation; no serialized Plan work occurs in Play.
        RegionBinding::admit(plan, &binding.active, &binding.region, binding.domain)?;
        fixed.validate().map_err(|_| DomainRefusal::WrongBinding)?;
        if binding.active.host_id.as_str() != crate::identity::hex(&fixed.host_id)
            || binding.active.boot_id.as_str() != crate::identity::hex(&fixed.boot_id)
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment.host_id == binding.active.host_id
                    && fragment.boot_id == binding.active.boot_id
            })
            .ok_or(DomainRefusal::WrongBinding)?;
        if fragment.offer_generation.0 != fixed.generation {
            return Err(DomainRefusal::WrongBinding);
        }
        let region = fragment
            .execution_regions
            .iter()
            .find(|region| region.region_id == binding.region)
            .ok_or(DomainRefusal::WrongBinding)?;
        let mut placements = fragment.placements.iter().filter(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::TEXT_PRESENTATION_KIND
                && region.admitted_placements.contains(&placement.placement_id)
        });
        let placement = placements.next().ok_or(DomainRefusal::WrongBinding)?;
        if placements.next().is_some()
            || placement.implementation_id.as_str() != TEXT_PRESENTATION_IMPLEMENTATION
            || !placement.authority.is_empty()
        {
            // This reviewed local presentation contract has no external-subject grant.
            // Other effect contracts must provide their own exact authority admission.
            return Err(DomainRefusal::WrongBinding);
        }
        let capability = fixed
            .capabilities
            .iter()
            .find(|capability| {
                capability.kind == placement.kind_id.as_str()
                    && capability.implementation == placement.implementation_id.as_str()
                    && capability.required_base == BaseKind::Serial
            })
            .ok_or(DomainRefusal::WrongBinding)?;
        let provider = fixed
            .capability_provider(capability)
            .map_err(|_| DomainRefusal::WrongBinding)?;
        fixed
            .require_base_provider(provider)
            .map_err(|_| DomainRefusal::WrongBinding)?;
        let selected = placement.base.as_ref().ok_or(DomainRefusal::WrongBinding)?;
        if selected.base_id.as_str() != crate::identity::hex(&provider.base_id)
            || selected.provider_instance_id.as_str()
                != crate::identity::hex(&provider.provider_instance_id)
            || selected.provider_generation != provider.provider_generation
            || selected.implementation_id.as_str()
                != crate::ordinary_base::SERIAL_PROVIDER_IMPLEMENTATION
            || selected.mechanism_family.as_str() != BaseKind::Serial.as_str()
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let [call] = placement.host_calls.as_slice() else {
            return Err(DomainRefusal::WrongBinding);
        };
        if Some(call.contract_id.as_str()) != capability.host_call
            || call.maximum_in_flight != 1
            || call.maximum_input_bytes == 0
            || call.maximum_input_bytes > capability.maximum_input_bytes
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let mut resources = placement.resources.iter().filter(|resource| {
            resource.class_id.as_str() == "conduit.resource/presentation-slot@1"
        });
        let resource = resources.next().ok_or(DomainRefusal::WrongBinding)?;
        if resources.next().is_some() || resource.units != 1 || resource.protected.is_some() {
            return Err(DomainRefusal::WrongBinding);
        }
        let pool = fixed
            .resources
            .iter()
            .enumerate()
            .find(|(index, offered)| {
                offered.base == BaseKind::Serial
                    && offered.class == resource.class_id.as_str()
                    && resource.pool_id.as_str()
                        == alloc::format!("conduitos-pool-{index}-{}", offered.base.as_str())
                    && resource.units <= offered.capacity
            })
            .ok_or(DomainRefusal::WrongBinding)?;
        let _ = pool;
        Ok(Self {
            scope: KernelCapabilityScope {
                host: fixed.host_id,
                boot: fixed.boot_id,
                plan: parse_identity(binding.active.plan_id.as_str())?,
                play: parse_identity(binding.active.active_play_id.as_str())?,
                implementation: identity(
                    b"implementation",
                    &[
                        placement.implementation_id.as_str().as_bytes(),
                        placement.artifact_id.as_str().as_bytes(),
                    ],
                ),
                base: provider.base_id,
                base_generation: u32::try_from(provider.provider_generation)
                    .map_err(|_| DomainRefusal::WrongBinding)?,
                resource: identity(b"resource", &[resource.pool_id.as_str().as_bytes()]),
                resource_generation: u32::try_from(fixed.generation)
                    .map_err(|_| DomainRefusal::WrongBinding)?,
                operation: SERIAL_PRESENT_OPERATION,
                subject: identity(
                    b"subject",
                    &[
                        placement.placement_id.as_str().as_bytes(),
                        placement.gear_id.as_str().as_bytes(),
                        placement.kind_id.as_str().as_bytes(),
                    ],
                ),
                authority: identity(
                    b"local-selected-presentation",
                    &[
                        placement.capability_id.as_str().as_bytes(),
                        call.contract_id.as_str().as_bytes(),
                    ],
                ),
                maximum_parameter_bytes: call.maximum_input_bytes,
                maximum_work_units: 1,
                maximum_in_flight: 1,
                maximum_operations: 1,
            },
            provider,
        })
    }

    /// The caller supplies current Root facts, never values from the user frame.
    pub fn current(
        &self,
        binding: &RegionBinding,
        provider_generation: Option<u64>,
    ) -> Result<KernelCapabilityScope, DomainRefusal> {
        if provider_generation != Some(self.provider.provider_generation)
            || parse_identity(binding.active.plan_id.as_str())? != self.scope.plan
            || parse_identity(binding.active.active_play_id.as_str())? != self.scope.play
            || parse_identity(binding.active.host_id.as_str())? != self.scope.host
            || parse_identity(binding.active.boot_id.as_str())? != self.scope.boot
        {
            return Err(DomainRefusal::WrongBinding);
        }
        Ok(self.scope)
    }
}

fn identity(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"conduit.conduitos/domain-effect-scope@1");
    digest.update((domain.len() as u32).to_le_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u32).to_le_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

fn parse_identity(identity: &str) -> Result<[u8; 32], DomainRefusal> {
    if identity.len() != 64 {
        return Err(DomainRefusal::WrongBinding);
    }
    let mut bytes = [0; 32];
    for (index, pair) in identity.as_bytes().chunks_exact(2).enumerate() {
        let digit = |byte| match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(DomainRefusal::WrongBinding),
        };
        bytes[index] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "domain_serial_scope/tests.rs"]
mod tests;
