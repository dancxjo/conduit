//! Exact local Body preparation using the installed browser session and kernel.
use super::*;
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_core::{
    ResourceAdmissionItem, ResourceAdmissionOwner, ResourceAdmissionRequest, ResourceObservation,
};
use conduit_plan_lowering::fragment_set::{lower_local_fragment_set, FragmentSetBounds};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BodyStartRequest {
    pub wake: Wake,
    pub plan: BodyPlan,
    pub play_sequence: u64,
    /// Supplied by the trusted page Host adapter, not inferred from offers.
    pub observations: Vec<ResourceObservation>,
}

#[derive(serde::Serialize)]
pub(super) struct BodyStarted {
    pub schema: &'static str,
    pub play: BodyPlayIdentity,
    pub wake_at_start: Wake,
    pub progress: TourProgress,
}

pub(super) fn prepare(request: BodyStartRequest) -> Result<(TourSession, BodyStarted), String> {
    use crate::installed_browser::*;
    request
        .plan
        .validate_for(&request.wake)
        .map_err(|error| format!("Body Plan: {error:?}"))?;
    let fragments = request
        .plan
        .forms
        .iter()
        .map(|part| {
            if part.plan.fragments.len() != 1 {
                return Err("browser Body requires one local fragment per Form".to_string());
            }
            Ok(part.plan.fragments[0].clone())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let first = fragments.first().ok_or("empty Body workload")?;
    let host =
        crate::installed_browser::advertisement(first.host_id.clone(), first.boot_id.clone());
    if request.observations.len() > host.resources.len() {
        return Err("Body resource observations exceed the installed resource bound".into());
    }
    let lowered = lower_local_fragment_set(
        &fragments.iter().collect::<Vec<_>>(),
        conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE,
        FragmentSetBounds {
            fragments: conduit_body::MAX_BODY_FORMS as u16,
            nodes: MAXIMUM_BROWSER_GEARS as u16,
            cords: MAXIMUM_BROWSER_CORDS as u16,
            queue_slots: BROWSER_QUEUE_SLOTS as u16,
            value_bytes: BROWSER_TOTAL_VALUE_BYTES,
            sign_items: BROWSER_SIGN_ITEMS,
            sign_bytes: u32::from(BROWSER_SIGN_ITEMS)
                * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
        },
    )
    .map_err(|error| format!("Body lowering: {error:?}"))?;
    let mut requests = Vec::new();
    let mut instances = BTreeMap::<conduit_core::CapabilityId, usize>::new();
    for fragment in &fragments {
        if fragment.host_id != host.host_id
            || fragment.boot_id != host.boot_id
            || fragment.offer_generation != host.offer_generation
        {
            return Err("Body fragment differs from the current browser Host".into());
        }
        for gear in &fragment.placements {
            let offer = host
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == gear.capability_id)
                .ok_or("Body capability is not installed")?;
            if gear.host_id != host.host_id
                || gear.boot_id != host.boot_id
                || gear.offer_generation != host.offer_generation
                || gear.implementation_id != offer.implementation.implementation_id
                || gear.kind_id != offer.kind_id
                || !gear.authority.is_empty()
                || !offer.authority_requirements.is_empty()
            {
                return Err(
                    "Body placement does not match the current supported browser offer".into(),
                );
            }
            let count = instances.entry(gear.capability_id.clone()).or_default();
            *count += 1;
            if *count > usize::from(offer.limits.max_active_instances) {
                return Err("Body capability instance limit exceeded".into());
            }
            if gear.resources.len() != offer.resource_requirements.len() {
                return Err("Body resource requirement count differs".into());
            }
            if offer.resource_requirements.iter().any(|requirement| {
                gear.resources
                    .iter()
                    .filter(|binding| binding.class_id == requirement.class_id)
                    .count()
                    != 1
            }) {
                return Err("Body resource requirement is missing or duplicated".into());
            }
            if gear.resources.is_empty() {
                continue;
            }
            let items = gear
                .resources
                .iter()
                .map(|binding| {
                    let requirement = offer
                        .resource_requirements
                        .iter()
                        .find(|requirement| requirement.class_id == binding.class_id)
                        .ok_or("Body resource is not required by its offer")?;
                    Ok(ResourceAdmissionItem {
                        requirement: requirement.clone(),
                        binding: binding.clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            requests.push(ResourceAdmissionRequest {
                plan_id: fragment.plan_id.clone(),
                placement_id: gear.placement_id.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                items,
            });
        }
    }
    let scheduler = engine::preparation::prepare_partition_scheduler(
        &fragments
            .iter()
            .zip(&lowered.partitions)
            .collect::<Vec<_>>(),
    )?;
    let mut resources = ResourceAdmissionOwner::new(host.clone());
    if !requests.is_empty() {
        resources
            .admit_batch(requests, &request.observations)
            .map_err(|error| error.to_string())?;
    }
    let play = BodyPlayIdentity::bind(&request.plan, request.play_sequence);
    let sign = |sequence| {
        bind_sign(
            &host.host_id,
            &host.boot_id,
            Some(&play.active_play_id),
            sequence,
        )
        .sign_id
    };
    let wake_at_start = request
        .wake
        .body_plan_ready(&request.plan, sign(0))
        .and_then(|wake| wake.body_play_started(&request.plan, &play, sign(1)))
        .map_err(|error| format!("Body start lifecycle: {error:?}"))?;
    let mut session = TourSession {
        _resource_admissions: Some(resources),
        cancellation: None,
        scheduler,
        pending: Vec::with_capacity(BROWSER_PENDING_REQUESTS),
        active_play_id: play.active_play_id.clone(),
        terminal_sign_sequence: 2,
        latest_presentation: None,
        host_id: host.host_id,
        boot_id: host.boot_id,
        realization: MorseRealization::Direct,
        expanded_gears: fragments
            .iter()
            .map(|fragment| {
                fragment
                    .placements
                    .iter()
                    .map(|gear| TourGearEvidence {
                        gear_id: gear.gear_id.as_str().into(),
                        kind_id: gear.kind_id.as_str().into(),
                        implementation_id: gear.implementation_id.as_str().into(),
                    })
                    .collect()
            })
            .collect(),
        realization_backs: fragments
            .iter()
            .map(|fragment| {
                fragment
                    .realization_backs
                    .iter()
                    .map(|back| TourBackEvidence {
                        invocation_path: back.invocation_path.clone(),
                        kind_id: back.kind_id.as_str().into(),
                        checked_form_id: back.checked_form_id.as_str().into(),
                    })
                    .collect()
            })
            .collect(),
        fragments,
        source_interaction: None,
        timer_completions: 0,
        manifestation_completions: 0,
    };
    let progress = session.poll_effect()?;
    Ok((
        session,
        BodyStarted {
            schema: "conduit.browser/body-started@1",
            play,
            wake_at_start,
            progress,
        },
    ))
}

#[cfg(test)]
#[path = "body_start_tests.rs"]
pub(in crate::form_runner) mod tests;
