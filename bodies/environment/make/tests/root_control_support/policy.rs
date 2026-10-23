//! Root-produced policy under CPU Boot fixtures, never a physical permission.
use super::*;
use conduit_environment_make::{NativeOfferRefusal as E, NativeSoftwareSourceRecipe};
use serde_json::json;
pub(super) fn round_trip(
    source: &conduit_environment_runtime::construction::ApplianceSource,
    host: &c::HostAdvertisement,
    grants: [c::AuthorityGrant; 2],
) -> [c::AuthorityGrant; 2] {
    let recipe = NativeSoftwareSourceRecipe::prepare(source).unwrap();
    let reply = json!({
        "event": "native-appliance-policy", "host_id": host.host_id,
        "boot_id": host.boot_id, "offer_generation": host.offer_generation,
        "source_identity": recipe.spec().source.expanded_plot_digest,
        "count": recipe.cadence().count, "period_ms": recipe.cadence().period_ms,
        "authority_grants": grants, "capability_possession_issued": false,
        "body_installed": false, "native_plan_installed": false,
    });
    let observed = recipe
        .observe_appliance_policy(host, &serde_json::to_vec(&reply).unwrap())
        .unwrap();
    assert_eq!(observed, grants);
    for (field, value, expected) in [
        ("boot_id", json!("stale/boot"), E::Scope),
        ("offer_generation", json!(999), E::Scope),
        ("source_identity", json!(vec![0; 32]), E::Source),
        ("count", json!(1), E::Cadence),
        ("period_ms", json!(1), E::Cadence),
        ("capability_possession_issued", json!(true), E::Installed),
        ("body_installed", json!(true), E::Installed),
        ("native_plan_installed", json!(true), E::Installed),
    ] {
        let mut changed = reply.clone();
        changed[field] = value;
        assert_eq!(
            recipe.observe_appliance_policy(host, &serde_json::to_vec(&changed).unwrap()),
            Err(expected)
        );
    }
    for field in [
        "host_id",
        "boot_id",
        "contract_id",
        "host_call_contract_id",
        "subject_kind",
        "capability_id",
    ] {
        let mut changed = reply.clone();
        changed["authority_grants"][0][field] = json!("foreign/value");
        assert_eq!(
            recipe.observe_appliance_policy(host, &serde_json::to_vec(&changed).unwrap()),
            Err(E::Authority)
        );
    }
    let mut changed = reply.clone();
    changed["authority_grants"][1] = changed["authority_grants"][0].clone();
    assert_eq!(
        recipe.observe_appliance_policy(host, &serde_json::to_vec(&changed).unwrap()),
        Err(E::Authority)
    );
    let mut no_base = host.clone();
    no_base.bases.clear();
    assert_eq!(
        recipe.observe_appliance_policy(&no_base, &serde_json::to_vec(&reply).unwrap()),
        Err(E::Authority)
    );
    assert_eq!(
        recipe.observe_appliance_policy(host, &vec![b' '; 8193]),
        Err(E::Bounds)
    );
    assert_eq!(
        recipe.observe_appliance_policy(host, b"{}"),
        Err(E::Encoding)
    );
    observed
}
