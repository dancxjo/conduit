//! Replay retained physical reports through host-side composition. This does
//! not claim that historical reports establish present availability/authority.
use super::*;
use conduit_environment_make::{NativeOfferRefusal as E, NativeSoftwareSourceRecipe};
use serde_json::{json, Value};

fn reports() -> (c::HostAdvertisement, Value, c::HostAdvertisement) {
    let replies: Vec<Value> = serde_json::from_slice(include_bytes!(
        "../../../evidence/2026-10-23-primitive-equality-stack/physical-ready-replies.json"
    ))
    .unwrap();
    (
        serde_json::from_value(replies[0]["advertisement"].clone()).unwrap(),
        replies[1].clone(),
        serde_json::from_value(replies[2]["advertisement"].clone()).unwrap(),
    )
}

#[test]
fn retained_physical_offers_compose_exactly_without_inventing_native_facts() {
    let source = construction::compile_default().unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let (before, reply, after) = reports();
    let combined = recipe
        .observe_native_offers(&before, &serde_json::to_vec(&reply).unwrap(), &after)
        .unwrap();
    assert_eq!(combined.host_id, before.host_id);
    assert_eq!(combined.boot_id, before.boot_id);
    assert_eq!(combined.offer_generation, before.offer_generation);
    assert_eq!(combined.bases, before.bases);
    assert_eq!(combined.resources, before.resources);
    assert_eq!(combined.planner_capabilities, before.planner_capabilities);
    assert_eq!(combined.capabilities.len(), before.capabilities.len() + 13);
    assert_eq!(
        &combined.capabilities[..before.capabilities.len()],
        &before.capabilities
    );
    // Observing native offers does not supply Root authority to ordinary planning.
    let refusal = source
        .plan_local(&combined, &["conduit.base/local@1".into()], &[])
        .unwrap_err();
    assert!(
        matches!(
            refusal,
            conduit_planner::PlannerError::AuthorityGrantMissing(_)
        ),
        "{refusal:?}"
    );
}

#[test]
fn stale_scope_source_cadence_and_forged_software_refuse() {
    let source = construction::compile_default().unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let (before, reply, after) = reports();
    for (field, value, expected) in [
        ("event", json!("other"), E::Scope),
        ("host_id", json!("host/foreign"), E::Scope),
        ("boot_id", json!("boot/stale"), E::Scope),
        ("offer_generation", json!(9999), E::Scope),
        ("source_identity", json!(vec![0; 32]), E::Source),
        ("count", json!(1), E::Cadence),
        ("period_ms", json!(1), E::Cadence),
        ("body_installed", json!(true), E::Installed),
        ("native_plan_installed", json!(true), E::Installed),
        ("capabilities_bytes", json!(1), E::Software),
    ] {
        let mut changed = reply.clone();
        changed[field] = value;
        assert_eq!(
            recipe.observe_native_offers(&before, &serde_json::to_vec(&changed).unwrap(), &after),
            Err(expected),
            "{field}"
        );
    }
    for mode in 0..4 {
        let mut changed = reply.clone();
        let caps = changed["capabilities"].as_array_mut().unwrap();
        match mode {
            0 => {
                caps.pop();
            }
            1 => caps[1] = caps[0].clone(),
            2 => caps[0]["artifact_id"] = json!("other/artifact"),
            _ => caps[0]["kind_id"] = json!("time/tick"),
        }
        assert_eq!(
            recipe.observe_native_offers(&before, &serde_json::to_vec(&changed).unwrap(), &after),
            Err(E::Software)
        );
    }
    let mut changed = after.clone();
    changed.boot_id = "boot/stale".into();
    assert_eq!(
        recipe.observe_native_offers(&before, &serde_json::to_vec(&reply).unwrap(), &changed),
        Err(E::Scope)
    );
    changed = after.clone();
    changed.capabilities.pop();
    assert_eq!(
        recipe.observe_native_offers(&before, &serde_json::to_vec(&reply).unwrap(), &changed),
        Err(E::NativeChanged)
    );
    assert_eq!(
        recipe.observe_native_offers(&before, &vec![b' '; 65537], &after),
        Err(E::Bounds)
    );
    assert_eq!(
        recipe.observe_native_offers(&before, b"{}", &after),
        Err(E::Encoding)
    );
    let mut duplicate = reply.clone();
    duplicate["unknown"] = json!(true);
    assert_eq!(
        recipe.observe_native_offers(&before, &serde_json::to_vec(&duplicate).unwrap(), &after),
        Err(E::Encoding)
    );
    let mut collision = before.clone();
    collision
        .capabilities
        .push(serde_json::from_value(reply["capabilities"][0].clone()).unwrap());
    assert_eq!(
        recipe.observe_native_offers(&collision, &serde_json::to_vec(&reply).unwrap(), &collision),
        Err(E::Software)
    );
}
