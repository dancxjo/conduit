use crate::{plan_with_options, PlacementChoices, PlannerError, PlanningOptions};
use conduit_core::{
    ConnectionBase, HostAdvertisement, Plan, PlannerCapabilityOffer, PlannerLimits,
    PlannerProfileId,
};
use conduit_form::CheckedForm;

pub const FULL_PLANNER_PROFILE: &str = "conduit.planner/full@1";
pub const BROWSER_PLANNER_PROFILE: &str = "conduit.planner/browser-wasm@1";

pub const FULL_PLANNER_LIMITS: PlannerLimits = PlannerLimits {
    maximum_host_advertisements: u16::MAX,
    maximum_gears: u16::MAX,
    maximum_connections: u16::MAX,
    maximum_authority_grants: u16::MAX,
    maximum_protected_resource_grants: u16::MAX,
    maximum_line_offers: u16::MAX,
};

/// Executes the shared deterministic planner under one capability advertised by
/// the calling host. The planner host is used only to select and enforce the
/// offer; it is not an input to plan construction or identity.
pub fn plan_with_advertised_profile(
    planner_host: &HostAdvertisement,
    profile_id: &PlannerProfileId,
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[ConnectionBase],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    let mut offers = planner_host
        .planner_capabilities
        .iter()
        .filter(|offer| &offer.profile_id == profile_id);
    let offer = offers.next().ok_or_else(|| {
        PlannerError::PlannerCapabilityNotAdvertised(format!(
            "host '{}' boot '{}' does not offer profile '{}'",
            planner_host.host_id.as_str(),
            planner_host.boot_id.as_str(),
            profile_id.as_str()
        ))
    })?;
    if offers.next().is_some() {
        return Err(PlannerError::PlannerCapabilityAmbiguous(format!(
            "host '{}' boot '{}' advertises profile '{}' more than once",
            planner_host.host_id.as_str(),
            planner_host.boot_id.as_str(),
            profile_id.as_str()
        )));
    }
    admit_request(offer, form, hosts, &options)?;
    plan_with_options(form, hosts, placements, bases, options)
}

fn admit_request(
    offer: &PlannerCapabilityOffer,
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    options: &PlanningOptions<'_>,
) -> Result<(), PlannerError> {
    admit_count(
        "host advertisements",
        hosts.len(),
        offer.limits.maximum_host_advertisements,
    )?;
    admit_count("gears", form.gears.len(), offer.limits.maximum_gears)?;
    admit_count(
        "connections",
        form.connections.len(),
        offer.limits.maximum_connections,
    )?;
    admit_count(
        "authority grants",
        options.authority_grants.len(),
        offer.limits.maximum_authority_grants,
    )?;
    admit_count(
        "protected resource grants",
        options.protected_resource_grants.len(),
        offer.limits.maximum_protected_resource_grants,
    )?;
    admit_count(
        "link bindings",
        options.line_offers.len(),
        offer.limits.maximum_line_offers,
    )
}

fn admit_count(name: &str, actual: usize, maximum: u16) -> Result<(), PlannerError> {
    if actual > usize::from(maximum) {
        return Err(PlannerError::PlannerLimitExceeded(format!(
            "profile input has {actual} {name}, above advertised maximum {maximum}"
        )));
    }
    Ok(())
}
