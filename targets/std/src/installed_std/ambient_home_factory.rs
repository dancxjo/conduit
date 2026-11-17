//! Exact Home realization eligibility and finite kernel budget. Projection and
//! escaped Face memory require their separate original IMAGE receipt.
use super::{
    ambient_home_back::AmbientHomeBack,
    back::{BackBudget, BackFactory, InstalledBack},
};
use conduit_core::{ConfigurationValue, PlannedGear};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: crate::ambient_home_offer::IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) static WALLPAPER_FACTORY: BackFactory = BackFactory {
    implementation_id: "std/ambient-wallpaper@1",
    budget,
    prepare,
};
pub(super) static CLOCK_WEATHER_FACTORY: BackFactory = BackFactory {
    implementation_id: "std/ambient-clock-weather@1",
    budget,
    prepare,
};

pub(super) static RESIDENT_WALLPAPER_FACTORY: BackFactory = BackFactory {
    implementation_id: "std/resident-ambient-wallpaper@1",
    budget,
    prepare,
};
pub(super) static RESIDENT_CLOCK_WEATHER_FACTORY: BackFactory = BackFactory {
    implementation_id: "std/resident-ambient-clock-weather@1",
    budget,
    prepare,
};

#[cfg(test)]
pub(in crate::installed_std) fn configuration(placement: &PlannedGear) -> Result<u32, String> {
    // Workspace belongs to the independently selected resource realization;
    // all semantic and executable identities are invariant under its amount.
    match configuration_policy(placement)? {
        conduit_ambient_display::AmbientUpdatePolicy::Finite(updates) => Ok(updates),
        _ => Err("resident contribution has no finite update allowance".into()),
    }
}
pub(in crate::installed_std) fn configuration_policy(
    placement: &PlannedGear,
) -> Result<conduit_ambient_display::AmbientUpdatePolicy, String> {
    let (aspect, resident) =
        conduit_std_offers::ambient_contribution_profile(placement.kind_id.as_str())
            .ok_or("unknown ambient contribution Kind")?;
    let workspace = crate::ambient_home_offer::workspace_bytes()?;
    let expected = if resident {
        conduit_std_offers::ambient_resident_aspect_std_offer(aspect, workspace)?
    } else {
        conduit_std_offers::ambient_aspect_std_offer(aspect, workspace)?
    };
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.semantic_contract != expected.semantic_contract
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.host_calls != expected.host_calls
        || placement.limits != expected.limits
        || placement.configuration.len() != if resident { 1 } else { 2 }
    {
        return Err("Home differs from its exact installed offer".into());
    }
    conduit_core::face_data_label(&placement.configuration)?;
    if resident {
        return Ok(conduit_ambient_display::AmbientUpdatePolicy::Resident);
    }
    let value = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "maximum-updates")
        .ok_or("Home maximum-updates is missing")?;
    let updates = match (&*value.key, &value.value) {
        ("maximum-updates", ConfigurationValue::U64(value)) => u32::try_from(*value)
            .ok()
            .filter(|n| *n != 0 && n.checked_mul(2).is_some()),
        _ => None,
    }
    .ok_or("Home requires a finite representable update bound")?;
    Ok(conduit_ambient_display::AmbientUpdatePolicy::Finite(
        updates,
    ))
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let policy = configuration_policy(placement)?;
    // Each observation: three committed Steps, two requests, two completions,
    // one input consumption. Fore ingress has its separate admitted Sign owner.
    // Two extra grants cover close/retirement; spare terminal lifecycle entries
    // are conservatively included. Observer-pressure polls do not invoke Back.
    let sign_items = match policy {
        conduit_ambient_display::AmbientUpdatePolicy::Finite(updates) => updates
            .checked_mul(8)
            .and_then(|n| n.checked_add(4))
            .and_then(|n| u16::try_from(n).ok())
            .ok_or("Home Sign envelope exceeds selected std profile")?,
        conduit_ambient_display::AmbientUpdatePolicy::Resident => 256,
    };
    Ok(BackBudget {
        // The original Fore Cord funds the only value. Both calls borrow it;
        // this sink produces no additional payload or Host Call output.
        value_items: 0,
        value_bytes: 0,
        // Existing request history retains first observation per (node, call).
        host_requests: 2,
        sign_items,
        maximum_value_bytes: conduit_semantic_catalog::AMBIENT_HOME_MAXIMUM_BYTES,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    // Validate the full finite budget before constructing any installed state.
    budget(placement)?;
    let back = match configuration_policy(placement)? {
        conduit_ambient_display::AmbientUpdatePolicy::Finite(updates) => {
            AmbientHomeBack::prepare(placement.placement_id.clone(), updates)?
        }
        conduit_ambient_display::AmbientUpdatePolicy::Resident => {
            AmbientHomeBack::prepare_resident(placement.placement_id.clone())?
        }
    };
    Ok(InstalledBack::AmbientHome(back))
}

#[cfg(test)]
#[path = "ambient_home_factory/tests.rs"]
mod tests;
