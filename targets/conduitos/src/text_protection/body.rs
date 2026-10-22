//! Prepared exact Body ownership; activation uses the real Body Play.
use super::*;
use crate::{domain_serial_scope::SerialScope, protected_region::BodyRegionBinding};
use conduit_body::{BodyPlan, BodyPlayIdentity, ResidentPlot};
use conduit_core::{BootId, ExecutionRegionId, HostId};

#[derive(Clone)]
pub(crate) struct BodyTextAdmission {
    plot: ResidentPlot,
    body_plan: conduit_core::PlanId,
    host: HostId,
    boot: BootId,
    region: ExecutionRegionId,
    serial: Option<SerialScope>,
    generation: u64,
    editor_capacity: Option<u64>,
}

impl BodyTextAdmission {
    pub fn prepare(
        plan: &BodyPlan,
        plot: &ResidentPlot,
        fixed: &crate::offer::HostOffer<'_>,
    ) -> Result<Self, MachineRunError> {
        plan.verify_seal()
            .map_err(|_| MachineRunError::KernelConstruction)?;
        fixed
            .validate()
            .map_err(|_| MachineRunError::KernelConstruction)?;
        let partition = plan
            .plots
            .iter()
            .find(|partition| &partition.plot == plot)
            .ok_or(MachineRunError::KernelConstruction)?;
        let [fragment] = partition.plan.fragments.as_slice() else {
            return Err(MachineRunError::KernelConstruction);
        };
        let [region] = fragment.execution_regions.as_slice() else {
            return Err(MachineRunError::KernelConstruction);
        };
        if fragment.host_id.as_str() != crate::identity::hex(&fixed.host_id)
            || fragment.boot_id.as_str() != crate::identity::hex(&fixed.boot_id)
            || fragment.offer_generation.0 != fixed.generation
        {
            return Err(MachineRunError::KernelConstruction);
        }
        for placement in &fragment.placements {
            if placement.kind_id.as_str() == conduit_semantic_catalog::KEYMAP_KIND {
                if placement.configuration.len() != 2
                    || !placement.configuration.iter().any(|entry| {
                        entry.key.as_str() == "layout"
                            && entry.value
                                == conduit_core::ConfigurationValue::Structured(
                                    conduit_human::default_keymap_configuration(),
                                )
                    })
                    || !placement.configuration.iter().any(|entry| {
                        entry.key.as_str() == "choices"
                            && entry.value
                                == conduit_core::ConfigurationValue::Structured(
                                    conduit_human::default_choices_configuration(),
                                )
                    })
                    || fragment.connections.iter().any(|cord| {
                        cord.sink_placement_id == placement.placement_id
                            && cord.sink_port_id.as_str() == "selection"
                    })
                {
                    return Err(MachineRunError::KernelConstruction);
                }
            }
        }
        let document = fragment.placements.iter().any(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::TEXT_EDIT_DOCUMENT_KIND
        });
        let mut editors = fragment.placements.iter().filter(|placement| {
            matches!(
                placement.kind_id.as_str(),
                conduit_semantic_catalog::TEXT_EDIT_KIND
                    | conduit_semantic_catalog::TEXT_EDIT_DOCUMENT_KIND
            )
        });
        let editor = editors.next();
        if editors.next().is_some() {
            return Err(MachineRunError::KernelConstruction);
        }
        let editor_capacity = editor
            .map(|p| {
                p.configuration
                    .iter()
                    .find_map(|field| match (&*field.key, &field.value) {
                        ("maximum-bytes", conduit_core::ConfigurationValue::U64(value))
                            if (1..=256).contains(value) =>
                        {
                            Some(*value)
                        }
                        _ => None,
                    })
                    .ok_or(MachineRunError::KernelConstruction)
            })
            .transpose()?;
        // A canonical document-only region has no serial presenter and receives
        // no serial scope or capability. Explicit serial consumers retain their
        // exact existing Base admission; absence is never an ambient grant.
        let serial = if document
            && !fragment.placements.iter().any(|placement| {
                placement.implementation_id.as_str() == crate::offer::TEXT_FACE_IMPLEMENTATION
            }) {
            None
        } else {
            Some(
                SerialScope::prepare_body(
                    plan,
                    plot,
                    &fragment.host_id,
                    &fragment.boot_id,
                    &region.region_id,
                    fixed,
                )
                .map_err(MachineRunError::ProtectionDomain)?,
            )
        };
        Ok(Self {
            plot: plot.clone(),
            body_plan: plan.plan_id.clone(),
            host: fragment.host_id.clone(),
            boot: fragment.boot_id.clone(),
            region: region.region_id.clone(),
            serial,
            generation: fixed.generation,
            editor_capacity,
        })
    }

    pub fn activate(
        &self,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
    ) -> Result<ProtectedText<BodyRegionBinding>, MachineRunError> {
        if plan.plan_id != self.body_plan {
            return Err(activation_refusal("plan-identity"));
        }
        let started = TextDomain::ticks();
        let domain = NEXT_DOMAIN
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| MachineRunError::KernelConstruction)?;
        let binding = BodyRegionBinding::admit(
            plan,
            play,
            &self.plot,
            &self.host,
            &self.boot,
            &self.region,
            ProtectionDomainId(domain),
        )
        .map_err(MachineRunError::ProtectionDomain)?;
        let partition = plan
            .plots
            .iter()
            .find(|p| p.plot == self.plot)
            .ok_or_else(|| activation_refusal("partition"))?;
        let region = partition
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.execution_regions)
            .find(|r| r.region_id == self.region)
            .ok_or_else(|| activation_refusal("region"))?;
        if region.execution_profile_id.as_str() != crate::ordinary_plan::PROTECTED_REGION_PROFILE
            || !region.isolation_required
            || !region.preemption_required
        {
            return Err(activation_refusal("region-contract"));
        }
        let mut capabilities = capability_table(self.generation).inspect_err(|_| {
            let _ = activation_refusal("capability-table");
        })?;
        let serial = self
            .serial
            .map(|prepared| {
                let scope = prepared
                    .activate_body(plan, &binding)
                    .map_err(MachineRunError::ProtectionDomain)?;
                let handle = capabilities
                    .issue(binding.domain, scope.scope)
                    .map_err(MachineRunError::ProtectionCapability)?;
                Ok::<_, MachineRunError>(SerialBinding { scope, handle })
            })
            .transpose()?;
        let heap_bytes = body_binding_heap(&binding)
            .checked_mul(2)
            .ok_or(MachineRunError::KernelConstruction)?;
        let root_bytes = u32::try_from(
            core::mem::size_of::<ProtectedText<BodyRegionBinding>>()
                + heap_bytes
                + core::mem::size_of::<
                    [Option<PureKeyboardOutput>; crate::native_workset::NATIVE_PLOT_CAPACITY],
                >(),
        )
        .map_err(|_| MachineRunError::KernelConstruction)?;
        if TextDomain::RESERVED_BYTES
            .checked_add(root_bytes)
            .is_none_or(|bytes| bytes > region.requirements.runtime_memory_bytes)
        {
            return Err(MachineRunError::ProtectionDomain(
                crate::protected_region::DomainRefusal::InvalidMemory,
            ));
        }
        let mut backend = TextDomain::install().map_err(MachineRunError::ProtectionDomain)?;
        backend.preparation_cost(started, root_bytes);
        let mut protected = ProtectedText {
            current: binding.clone(),
            region: ProtectedRegion::installed(binding, backend),
            capabilities,
            serial,
            diagnostic_fixture: false,
            editor: false,
        };
        if let Some(maximum) = self.editor_capacity {
            protected.reset_editor(maximum)?;
        } else {
            protected.reset_keymap()?;
        }
        Ok(protected)
    }
}

fn activation_refusal(stage: &str) -> MachineRunError {
    crate::arch::early_write(b"CONDUIT_TEXT_ACTIVATION_REFUSAL ");
    crate::arch::early_write(stage.as_bytes());
    crate::arch::early_write(b"\n");
    MachineRunError::KernelConstruction
}

fn body_binding_heap(binding: &BodyRegionBinding) -> usize {
    binding.active.active_play_id.0.capacity()
        + binding.active.body_id.as_str().len()
        + binding.active.wake_id.as_str().len()
        + binding.active.plan_id.0.capacity()
        + binding.plot.source_document_id.0.capacity()
        + binding.plot.checked_plot_id.0.capacity()
        + binding.partition_plan.0.capacity()
        + binding.host.0.capacity()
        + binding.boot.0.capacity()
        + binding.region.0.capacity()
}
