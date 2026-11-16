//! Exact proposed Body placement and actual Started custody for one provision.
use super::*;
use conduit_body_make::womb_runtime::{WombRequest, WOMB_KIND};
use conduit_core::{FragmentId, PlanId, PlannedGear};

#[derive(PartialEq)]
pub(super) struct Binding {
    pub(super) plan: PlanId,
    pub(super) fragment: FragmentId,
    pub(super) placement: PlannedGear,
    pub(super) request: WombRequest,
    pub(super) play: Option<BodyPlayIdentity>,
}

pub(crate) fn prepared_request(
    plan: &conduit_body::BodyPlan,
    fragment: &conduit_core::PlanFragment,
    gear: &PlannedGear,
) -> Result<WombRequest, String> {
    PROVISION.with(|slot| {
        let slot = slot.borrow();
        let provision = slot
            .as_ref()
            .ok_or("Womb has no initialized private provision")?;
        let binding = provision
            .binding
            .as_ref()
            .ok_or("Womb Back was not prepared")?;
        if binding.plan != plan.plan_id
            || binding.fragment != fragment.fragment_id
            || &binding.placement != gear
            || binding.play.is_some()
        {
            return Err("Womb Fore differs from its exact prepared Body realization".into());
        }
        Ok(binding.request)
    })
}

pub(crate) fn bind(placement: &PlannedGear) -> Result<(), String> {
    super::super::BODY.with(|body| {
        let body = body
            .try_borrow()
            .map_err(|_| "Womb cannot prepare during a Body mutation")?;
        let body = body
            .as_ref()
            .ok_or("Womb requires the actual local Body owner")?;
        let realization = body
            .realization()
            .ok_or("Womb requires an actual Body proposal")?;
        realization
            .plan
            .validate_for(&realization.wake)
            .map_err(|error| format!("Womb Body Plan: {error:?}"))?;
        if realization.play.is_some() {
            return Err("Womb cannot initialize a new Back during a Play".into());
        }
        PROVISION.with(|slot| {
            let mut slot = slot.borrow_mut();
            let provision = slot
                .as_mut()
                .ok_or("Womb has no private issuer provision")?;
            require_issuer(body, &provision.host, &provision.boot)
                .map_err(|error| error.message)?;
            let resource = provision.resource();
            let mut selected = None;
            for plot in &realization.plan.plots {
                for fragment in &plot.plan.fragments {
                    for gear in &fragment.placements {
                        if gear.kind_id.as_str() == WOMB_KIND {
                            if selected.replace((fragment, gear)).is_some() {
                                return Err("one provision can bind only one Womb operation".into());
                            }
                        } else if gear.host_id == provision.host
                            && gear.resources.iter().any(|r| r.pool_id == resource.pool_id)
                        {
                            return Err(
                                "another operation consumes the private Womb resource".into()
                            );
                        }
                    }
                }
            }
            if realization
                .plan
                .mask_topologies
                .iter()
                .flat_map(|mask| &mask.chains)
                .flat_map(|chain| &chain.plan.fragments)
                .flat_map(|fragment| &fragment.placements)
                .any(|gear| gear.kind_id.as_str() == WOMB_KIND)
            {
                return Err("a Mask cannot own this Womb provision".into());
            }
            let (fragment, gear) = selected.ok_or("the current Body proposal contains no Womb")?;
            if gear != placement
                || fragment.host_id != provision.host
                || fragment.boot_id != provision.boot
                || gear.host_id != fragment.host_id
                || gear.boot_id != fragment.boot_id
                || gear.offer_generation != fragment.offer_generation
                || gear.resources.len() != 1
                || gear.resources[0].pool_id != resource.pool_id
                || gear.resources[0].class_id != resource.class_id
                || gear.resources[0].units != 1
            {
                return Err(
                    "Womb selection differs from the exact owned Body fragment/resource".into(),
                );
            }
            let current = body.evidence().membership.parts.iter().any(|part| {
                part.state == conduit_body::MembershipState::Admitted
                    && part.current.as_ref().is_some_and(|current| {
                        current.host_id == gear.host_id
                            && current.boot_id == gear.boot_id
                            && current.offer_generation == gear.offer_generation
                    })
            });
            if !current {
                return Err("Womb selection names a stale Host incarnation".into());
            }
            let bytes = serde_json::to_vec(&(
                "conduit.womb/body-request@1",
                provision.id,
                &realization.plan.plan_id,
                &fragment.fragment_id,
                gear,
            ))
            .map_err(|error| error.to_string())?;
            let binding = Binding {
                plan: realization.plan.plan_id.clone(),
                fragment: fragment.fragment_id.clone(),
                placement: gear.clone(),
                request: WombRequest(Sha256::digest(bytes).into()),
                play: None,
            };
            if let Some(prior) = &provision.binding {
                if prior != &binding {
                    return Err(
                        "private Womb provision is already bound to another realization".into(),
                    );
                }
            } else {
                provision.binding = Some(binding);
            }
            Ok(())
        })
    })
}

/// Called only by Workspace's actual-kernel Started acknowledgement path.
pub(in crate::workspace_runtime) fn confirm_started(
    body: &BodyLifecycleSession,
) -> Result<(), Refusal> {
    PROVISION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(provision) = slot.as_mut() else {
            return Ok(());
        };
        let Some(binding) = provision.binding.as_mut() else {
            return Ok(());
        };
        let realization = body
            .realization()
            .ok_or("Womb Started has no Body realization")?;
        if binding.plan != realization.plan.plan_id {
            return Ok(());
        }
        let play = realization
            .play
            .as_ref()
            .ok_or("Womb requires an actual Started Play")?;
        if !play.validate_for(&realization.plan)
            || binding.play.as_ref().is_some_and(|prior| prior != play)
        {
            return Err(Refusal::new(
                "Womb.StalePlay",
                "Started differs from the bound Womb Play",
            ));
        }
        binding.play = Some(play.clone());
        Ok(())
    })
}

pub(in crate::workspace_runtime) fn request_for_started(
    body: &BodyLifecycleSession,
    host: &HostId,
    boot: &BootId,
    id: [u8; 32],
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, host, boot)?;
    PROVISION.with(|slot| {
        let slot = slot.borrow();
        let provision = slot.as_ref().ok_or("Womb has no private provision")?;
        if provision.id != id || &provision.host != host || &provision.boot != boot {
            return Err(Refusal::new("Womb.StaleReservation", "Request names another Womb provision"));
        }
        let binding = provision.binding.as_ref().ok_or_else(|| Refusal::new("Womb.NotStarted", "Prepare the actual Womb Body Play first"))?;
        let play = binding.play.as_ref().ok_or_else(|| Refusal::new("Womb.NotStarted", "Record the actual kernel Started acknowledgement first"))?;
        if body.realization().is_none_or(|realization| realization.plan.plan_id != binding.plan || realization.play.as_ref() != Some(play)) {
            return Err(Refusal::new("Womb.StalePlay", "The bound Womb Play is no longer current"));
        }
        encode(&serde_json::json!({"schema":"conduit.womb/body-request@1", "request":binding.request.0, "play":play, "fragment_id":binding.fragment, "placement_id":binding.placement.placement_id}))
    })
}
