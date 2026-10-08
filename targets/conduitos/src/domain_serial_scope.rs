//! Root-owned exact admission for the ordinary text region's serial effect.
use crate::{
    machine::BaseKind,
    offer::{
        BaseProviderBinding, HostOffer, INDICATOR_PRESENTATION_IMPLEMENTATION,
        TEXT_PRESENTATION_IMPLEMENTATION,
    },
    protected_region::{BodyRegionBinding, DomainRefusal, RegionBinding},
    protection_domain::KernelCapabilityScope,
};
use conduit_core::Plan;
use sha2::{Digest, Sha256};

pub const SERIAL_PRESENT_OPERATION: u32 = 7;
pub const COUNT_PRESENT_OPERATION: u32 = 11;

#[derive(Clone, Copy)]
pub struct SerialScope {
    pub scope: KernelCapabilityScope,
    pub provider: BaseProviderBinding,
    body_owner: Option<[u8; 32]>,
}

impl SerialScope {
    /// Validate the selected Body partition before activation. No handle is
    /// issued here; the active Body Play replaces the empty play field later.
    pub fn prepare_body(
        plan: &conduit_body::BodyPlan,
        plot: &conduit_body::ResidentPlot,
        host: &conduit_core::HostId,
        boot: &conduit_core::BootId,
        region: &conduit_core::ExecutionRegionId,
        fixed: &HostOffer<'_>,
    ) -> Result<Self, DomainRefusal> {
        plan.verify_seal()
            .map_err(|_| DomainRefusal::WrongBinding)?;
        let partition = plan
            .plots
            .iter()
            .find(|partition| &partition.plot == plot)
            .ok_or(DomainRefusal::WrongBinding)?;
        let mut prepared = Self::admit_selected(
            &partition.plan,
            host,
            boot,
            region,
            (
                identity(b"body-plan", &[plan.plan_id.as_str().as_bytes()]),
                [0; 32],
            ),
            fixed,
            Presentation::Text,
        )?;
        prepared.body_owner = Some(body_owner(plot, region));
        Ok(prepared)
    }

    pub fn activate_body(
        &self,
        plan: &conduit_body::BodyPlan,
        binding: &BodyRegionBinding,
    ) -> Result<Self, DomainRefusal> {
        if BodyRegionBinding::admit(
            plan,
            &binding.active,
            &binding.plot,
            &binding.host,
            &binding.boot,
            &binding.region,
            binding.domain,
        )? != *binding
            || self.scope.plan != identity(b"body-plan", &[plan.plan_id.as_str().as_bytes()])
            || self.scope.host != parse_identity(binding.host.as_str())?
            || self.scope.boot != parse_identity(binding.boot.as_str())?
            || self.scope.play != [0; 32]
            || self.body_owner != Some(body_owner(&binding.plot, &binding.region))
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let mut active = *self;
        active.scope.play = identity(
            b"body-play",
            &[binding.active.active_play_id.as_str().as_bytes()],
        );
        Ok(active)
    }

    pub fn admit(
        plan: &Plan,
        binding: &RegionBinding,
        fixed: &HostOffer<'_>,
    ) -> Result<Self, DomainRefusal> {
        // Admission runs during preparation; no serialized Plan work occurs in Play.
        RegionBinding::admit(plan, &binding.active, &binding.region, binding.domain)?;
        let mut selected = Self::admit_selected(
            plan,
            &binding.active.host_id,
            &binding.active.boot_id,
            &binding.region,
            (
                parse_identity(binding.active.plan_id.as_str())?,
                parse_identity(binding.active.active_play_id.as_str())?,
            ),
            fixed,
            Presentation::Text,
        )?;
        // This ordinary literal composition emits one value. The presentation
        // kind's larger configured ceiling does not grant additional effects.
        selected.scope.maximum_operations = 1;
        Ok(selected)
    }

    /// Admit the independently selected indicator effect of an ordinary Morse Play.
    pub fn admit_indicator(
        plan: &Plan,
        binding: &RegionBinding,
        fixed: &HostOffer<'_>,
    ) -> Result<Self, DomainRefusal> {
        RegionBinding::admit(plan, &binding.active, &binding.region, binding.domain)?;
        Self::admit_selected(
            plan,
            &binding.active.host_id,
            &binding.active.boot_id,
            &binding.region,
            (
                parse_identity(binding.active.plan_id.as_str())?,
                parse_identity(binding.active.active_play_id.as_str())?,
            ),
            fixed,
            Presentation::Indicator,
        )
    }

    /// Count's semantic input is eight bytes; the domain renders at most twenty
    /// decimal digits before crossing this separately bounded physical gate.
    pub fn admit_count(
        plan: &Plan,
        binding: &RegionBinding,
        fixed: &HostOffer<'_>,
    ) -> Result<Self, DomainRefusal> {
        RegionBinding::admit(plan, &binding.active, &binding.region, binding.domain)?;
        Self::admit_selected(
            plan,
            &binding.active.host_id,
            &binding.active.boot_id,
            &binding.region,
            (
                parse_identity(binding.active.plan_id.as_str())?,
                parse_identity(binding.active.active_play_id.as_str())?,
            ),
            fixed,
            Presentation::Count,
        )
    }

    pub fn admit_body(
        plan: &conduit_body::BodyPlan,
        binding: &BodyRegionBinding,
        fixed: &HostOffer<'_>,
    ) -> Result<Self, DomainRefusal> {
        let admitted = BodyRegionBinding::admit(
            plan,
            &binding.active,
            &binding.plot,
            &binding.host,
            &binding.boot,
            &binding.region,
            binding.domain,
        )?;
        if admitted != *binding {
            return Err(DomainRefusal::WrongBinding);
        }
        let partition = plan
            .plots
            .iter()
            .find(|partition| partition.plot == binding.plot)
            .ok_or(DomainRefusal::WrongBinding)?;
        let mut admitted = Self::admit_selected(
            &partition.plan,
            &binding.host,
            &binding.boot,
            &binding.region,
            (
                identity(b"body-plan", &[plan.plan_id.as_str().as_bytes()]),
                identity(
                    b"body-play",
                    &[binding.active.active_play_id.as_str().as_bytes()],
                ),
            ),
            fixed,
            Presentation::Text,
        )?;
        admitted.body_owner = Some(body_owner(&binding.plot, &binding.region));
        Ok(admitted)
    }

    fn admit_selected(
        plan: &Plan,
        host: &conduit_core::HostId,
        boot: &conduit_core::BootId,
        region_id: &conduit_core::ExecutionRegionId,
        (plan_id, play_id): ([u8; 32], [u8; 32]),
        fixed: &HostOffer<'_>,
        presentation: Presentation,
    ) -> Result<Self, DomainRefusal> {
        fixed.validate().map_err(|_| DomainRefusal::WrongBinding)?;
        if host.as_str() != crate::identity::hex(&fixed.host_id)
            || boot.as_str() != crate::identity::hex(&fixed.boot_id)
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| &fragment.host_id == host && &fragment.boot_id == boot)
            .ok_or(DomainRefusal::WrongBinding)?;
        if fragment.offer_generation.0 != fixed.generation {
            return Err(DomainRefusal::WrongBinding);
        }
        let region = fragment
            .execution_regions
            .iter()
            .find(|region| &region.region_id == region_id)
            .ok_or(DomainRefusal::WrongBinding)?;
        let mut placements = fragment.placements.iter().filter(|placement| {
            placement.kind_id.as_str() == presentation.kind()
                && region.admitted_placements.contains(&placement.placement_id)
        });
        let placement = placements.next().ok_or(DomainRefusal::WrongBinding)?;
        if placements.next().is_some()
            || placement.implementation_id.as_str() != presentation.implementation()
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
        if placement.kind_contract_revision.as_str() != capability.contract_revision
            || placement.artifact_id.as_str()
                != alloc::format!("conduitos-build/{}", capability.artifact_build)
        {
            return Err(DomainRefusal::WrongBinding);
        }
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
            || call.maximum_output_bytes > presentation.maximum_completion_bytes()
            || (matches!(presentation, Presentation::Count) && call.maximum_input_bytes != 8)
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
        let maximum_operations = match presentation {
            Presentation::Count if placement.configuration.is_empty() => 2,
            Presentation::Count => return Err(DomainRefusal::WrongBinding),
            Presentation::Indicator if placement.configuration.is_empty() => 1,
            Presentation::Indicator => return Err(DomainRefusal::WrongBinding),
            Presentation::Text => placement
                .configuration
                .iter()
                .find_map(|entry| match (entry.key.as_str(), &entry.value) {
                    ("maximum-values", conduit_core::ConfigurationValue::U64(value)) => {
                        u32::try_from(*value)
                            .ok()
                            .filter(|value| (1..=8).contains(value))
                    }
                    _ => None,
                })
                .ok_or(DomainRefusal::WrongBinding)?,
        };
        Ok(Self {
            scope: KernelCapabilityScope {
                host: fixed.host_id,
                boot: fixed.boot_id,
                plan: plan_id,
                play: play_id,
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
                operation: if matches!(presentation, Presentation::Count) {
                    COUNT_PRESENT_OPERATION
                } else {
                    SERIAL_PRESENT_OPERATION
                },
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
                maximum_parameter_bytes: match presentation {
                    Presentation::Count => 20,
                    _ => call.maximum_input_bytes,
                },
                maximum_work_units: 1,
                maximum_in_flight: 1,
                maximum_operations,
            },
            provider,
            body_owner: None,
        })
    }

    /// The caller supplies current Root facts, never values from the user frame.
    pub fn current_body(
        &self,
        binding: &BodyRegionBinding,
        provider_generation: Option<u64>,
    ) -> Result<KernelCapabilityScope, DomainRefusal> {
        if self.body_owner != Some(body_owner(&binding.plot, &binding.region))
            || provider_generation != Some(self.provider.provider_generation)
            || identity(b"body-plan", &[binding.active.plan_id.as_str().as_bytes()])
                != self.scope.plan
            || identity(
                b"body-play",
                &[binding.active.active_play_id.as_str().as_bytes()],
            ) != self.scope.play
            || parse_identity(binding.host.as_str())? != self.scope.host
            || parse_identity(binding.boot.as_str())? != self.scope.boot
        {
            return Err(DomainRefusal::WrongBinding);
        }
        Ok(self.scope)
    }

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

#[derive(Clone, Copy)]
enum Presentation {
    Text,
    Indicator,
    Count,
}

impl Presentation {
    fn kind(self) -> &'static str {
        match self {
            Self::Count => conduit_semantic_catalog::COUNT_PRESENTATION_KIND,
            Self::Text => conduit_semantic_catalog::TEXT_PRESENTATION_KIND,
            Self::Indicator => conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND,
        }
    }
    fn maximum_completion_bytes(self) -> u32 {
        match self {
            Self::Text => conduit_core::MAX_PRESENTATION_COMPLETION_BYTES,
            Self::Indicator | Self::Count => 0,
        }
    }
    fn implementation(self) -> &'static str {
        match self {
            Self::Count => crate::offer::COUNT_PRESENTATION_IMPLEMENTATION,
            Self::Text => TEXT_PRESENTATION_IMPLEMENTATION,
            Self::Indicator => INDICATOR_PRESENTATION_IMPLEMENTATION,
        }
    }
}

fn body_owner(
    plot: &conduit_body::ResidentPlot,
    region: &conduit_core::ExecutionRegionId,
) -> [u8; 32] {
    identity(
        b"body-region-owner",
        &[
            plot.source_document_id.as_str().as_bytes(),
            plot.checked_plot_id.as_str().as_bytes(),
            region.as_str().as_bytes(),
        ],
    )
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
    for (index, pair) in identity.as_bytes().as_chunks::<2>().0.iter().enumerate() {
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
