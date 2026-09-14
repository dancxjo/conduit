//! Separate exact Form partitions, one Body Plan, and combined resource bounds.
use alloc::{collections::BTreeMap, vec::Vec};
use conduit_body::{BodyFormPlan, BodyPlan, BodyPresenterTopology, Wake};
use conduit_core::{
    HostAdvertisement, KindId, PlacementId, Plan, PortId, ResourceClassId, ResourcePoolId,
};
use conduit_plan_lowering::fragment_set::{
    FragmentSetBounds, LoweredFragmentSet, lower_local_fragment_set,
};
use conduit_planner::{
    ConnectionQueueLimits, PlanningOptions, default_expanded_placements,
    plan_expanded_canonical_with_connection_limits,
};

use super::{WorksetRefusal, catalog};
use crate::{identity::BootIdentities, offer::HostOffer};

pub struct PreparedNativeWorkset {
    pub(super) advertisement: HostAdvertisement,
    pub(super) plan: BodyPlan,
    pub(super) lowered: LoweredFragmentSet,
    /// Exact initialized physical provider behind the logical deliveries.
    pub(super) keyboard: crate::keyboard_offer::KeyboardRealization,
    pub(super) input_owners: Vec<AdmittedFormInput>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedFormInput {
    pub form: conduit_body::ResidentForm,
    pub kind_id: KindId,
    pub placement_id: PlacementId,
    pub port_id: PortId,
    pub value_kind: KindId,
}

impl PreparedNativeWorkset {
    pub fn plan(&self) -> &BodyPlan {
        &self.plan
    }
    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }
    pub fn keyboard(&self) -> crate::keyboard_offer::KeyboardRealization {
        self.keyboard
    }
    pub fn input_owners(&self) -> &[AdmittedFormInput] {
        &self.input_owners
    }
    pub fn into_plan(self) -> BodyPlan {
        self.plan
    }

    pub(crate) fn with_presenters(
        mut self,
        wake: &Wake,
        presenter_topologies: Vec<BodyPresenterTopology>,
    ) -> Result<Self, WorksetRefusal> {
        self.plan =
            BodyPlan::seal_with_presenters(wake, self.plan.forms.clone(), presenter_topologies)
                .map_err(|_| WorksetRefusal::Plan)?;
        Ok(self)
    }
}

pub fn prepare(
    wake: &Wake,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedNativeWorkset, WorksetRefusal> {
    let profile = super::profile();
    if wake.workset.is_empty() || wake.workset.len() > profile.capacity {
        return Err(WorksetRefusal::WorksetBound);
    }
    for form in wake.workset.forms() {
        if !profile.contains(form)? {
            return Err(WorksetRefusal::UnknownForm);
        }
    }
    let (advertisement, keyboard) = host(identities, offer, build_id)?;
    let forms = plan_forms(wake.workset.forms(), &advertisement)?;
    let plan = BodyPlan::seal(wake, forms).map_err(|_| WorksetRefusal::Plan)?;
    let input_owners = plan
        .forms
        .iter()
        .map(admitted_form_input)
        .collect::<Result<Vec<_>, _>>()?;
    validate_combined(&advertisement, plan.forms.iter().map(|form| &form.plan))?;
    let lowered = lower_forms(&plan.forms)?;
    Ok(PreparedNativeWorkset {
        advertisement,
        plan,
        lowered,
        keyboard,
        input_owners,
    })
}

fn admitted_form_input(form: &BodyFormPlan) -> Result<AdmittedFormInput, WorksetRefusal> {
    let fragment = form.plan.fragments.first().ok_or(WorksetRefusal::Plan)?;
    let placement = fragment
        .placements
        .iter()
        .find(|placement| {
            matches!(
                placement.kind_id.as_str(),
                conduit_semantic_catalog::KEYBOARD_KIND
                    | conduit_semantic_catalog::APPLICATION_EVENT_SOURCE_KIND
            )
        })
        .ok_or(WorksetRefusal::Capability)?;
    let connection = fragment
        .connections
        .iter()
        .find(|connection| connection.source_placement_id == placement.placement_id)
        .ok_or(WorksetRefusal::Plan)?;
    let keyboard = placement.kind_id.as_str() == conduit_semantic_catalog::KEYBOARD_KIND
        && connection.source_port_id.as_str() == conduit_semantic_catalog::KEYBOARD_PORT
        && connection.value_kind.as_str() == conduit_human::KEY_EVENT_INFO_ID;
    let application = placement.kind_id.as_str()
        == conduit_semantic_catalog::APPLICATION_EVENT_SOURCE_KIND
        && connection.source_port_id.as_str() == conduit_semantic_catalog::APPLICATION_EVENT_PORT
        && connection.value_kind.as_str() == conduit_presentation::APPLICATION_EVENT_INFO_ID;
    if !keyboard && !application {
        return Err(WorksetRefusal::Plan);
    }
    Ok(AdmittedFormInput {
        form: form.form.clone(),
        kind_id: placement.kind_id.clone(),
        placement_id: placement.placement_id.clone(),
        port_id: connection.source_port_id.clone(),
        value_kind: connection.value_kind.clone(),
    })
}

/// Review exact Form planning and bounds without creating a Body or a Play.
pub fn review(
    form: super::NativeForm,
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<(), WorksetRefusal> {
    let (advertisement, _) = host(identities, offer, build_id)?;
    let form = catalog::resident(form)?;
    let forms = plan_forms(core::slice::from_ref(&form), &advertisement)?;
    validate_combined(&advertisement, forms.iter().map(|form| &form.plan))?;
    lower_forms(&forms)?;
    Ok(())
}

fn host(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<
    (
        HostAdvertisement,
        crate::keyboard_offer::KeyboardRealization,
    ),
    WorksetRefusal,
> {
    let keyboard = offer.keyboard.ok_or(WorksetRefusal::Host)?;
    let mut advertisement = crate::ordinary_plan::advertisement(identities, offer, build_id)
        .map_err(|_| WorksetRefusal::Host)?;
    crate::keyboard_text_plan::append_keymap_offer(&mut advertisement, build_id);
    let keyboard = super::keyboard_delivery::install(&mut advertisement, keyboard, build_id)?;
    // The prepared Body tables hold two independent keymaps and presenters.
    // Their existing resource requirements still reserve each instance.
    for capability in &mut advertisement.capabilities {
        if matches!(
            capability.kind_id.as_str(),
            conduit_semantic_catalog::KEYMAP_KIND
                | conduit_semantic_catalog::TEXT_PRESENTATION_KIND
        ) {
            capability.limits.max_active_instances = 2;
        }
    }
    advertisement
        .capabilities
        .push(super::text_state::offer(build_id));
    advertisement
        .capabilities
        .extend(super::application_delivery::offers(build_id));
    Ok((advertisement, keyboard))
}

fn plan_forms(
    identities: &[conduit_body::ResidentForm],
    advertisement: &HostAdvertisement,
) -> Result<Vec<BodyFormPlan>, WorksetRefusal> {
    let mut forms = Vec::with_capacity(identities.len());
    for identity in identities {
        let expanded = catalog::checked(catalog::resolve(identity)?)?;
        let hosts = core::slice::from_ref(advertisement);
        let placements =
            default_expanded_placements(&expanded, hosts).map_err(|_| WorksetRefusal::Plan)?;
        let mut limits = BTreeMap::new();
        for cord in &expanded.connections {
            let capability = |gear| {
                let choice = placements.by_gear.get(gear).ok_or(WorksetRefusal::Plan)?;
                advertisement
                    .capabilities
                    .iter()
                    .find(|offer| offer.capability_id == choice.capability_id)
                    .ok_or(WorksetRefusal::Capability)
            };
            let source = capability(&cord.source_gear_id)?;
            let sink = capability(&cord.sink_gear_id)?;
            limits.insert(
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: source
                        .limits
                        .max_queue_bytes
                        .min(sink.limits.max_queue_bytes),
                },
            );
        }
        let plan = plan_expanded_canonical_with_connection_limits(
            &expanded,
            hosts,
            &placements,
            &["conduit.base/local@1".into()],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_text::MAX_TEXT_BYTES,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &limits,
        )
        .map_err(|_| WorksetRefusal::Plan)?;
        forms.push(BodyFormPlan {
            form: identity.clone(),
            plan,
        });
    }
    Ok(forms)
}

fn lower_forms(forms: &[BodyFormPlan]) -> Result<LoweredFragmentSet, WorksetRefusal> {
    lower_local_fragment_set(
        &forms
            .iter()
            .map(|form| &form.plan.fragments[0])
            .collect::<Vec<_>>(),
        conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: 4,
            nodes: 14,
            cords: 10,
            queue_slots: 10,
            value_bytes: 32_768,
            sign_items: 1024,
            sign_bytes: 1024 * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
        },
    )
    .map_err(|_| WorksetRefusal::Lowering)
}

fn validate_combined<'a>(
    host: &HostAdvertisement,
    plans: impl Iterator<Item = &'a Plan>,
) -> Result<(), WorksetRefusal> {
    let mut resources = BTreeMap::<(ResourcePoolId, ResourceClassId), u32>::new();
    let mut instances = BTreeMap::new();
    for plan in plans {
        if plan.fragments.len() != 1 {
            return Err(WorksetRefusal::Plan);
        }
        let fragment = &plan.fragments[0];
        if fragment.host_id != host.host_id
            || fragment.boot_id != host.boot_id
            || fragment.offer_generation != host.offer_generation
        {
            return Err(WorksetRefusal::Host);
        }
        for placement in &fragment.placements {
            let offer = host
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
                .ok_or(WorksetRefusal::Capability)?;
            let count = instances
                .entry(placement.capability_id.clone())
                .or_insert(0_u32);
            *count = count.checked_add(1).ok_or(WorksetRefusal::Capability)?;
            if *count > u32::from(offer.limits.max_active_instances) {
                return Err(WorksetRefusal::Capability);
            }
            for binding in &placement.resources {
                let total = resources
                    .entry((binding.pool_id.clone(), binding.class_id.clone()))
                    .or_default();
                *total = total
                    .checked_add(binding.units)
                    .ok_or(WorksetRefusal::Resource)?;
                let pool = host
                    .resources
                    .iter()
                    .find(|pool| {
                        pool.pool_id == binding.pool_id && pool.class_id == binding.class_id
                    })
                    .ok_or(WorksetRefusal::Resource)?;
                if *total > pool.capacity_units {
                    return Err(WorksetRefusal::Resource);
                }
            }
        }
    }
    Ok(())
}
