//! Scope/pin authority are CPU fixtures; offer metadata comes from real retained
//! Source-compiled software. This is not a trusted transport or native execution.
use super::planning::fixture;
use conduit_environment_make::{
    DisplayOutputSource, NativeOfferRefusal as E, PreparedDisplayOutput,
};
#[test]
fn observed_output_requires_unchanged_native_scope_exact_source_and_permission() {
    let source = DisplayOutputSource::compile().unwrap();
    let (mut host, grant, mut software) = fixture(&source);
    host.capabilities
        .retain(|offer| offer.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND);
    let view = software
        .current_offers(&host.host_id, &host.boot_id, host.offer_generation)
        .unwrap();
    let capabilities: Vec<_> = view.iter().cloned().collect();
    let reply = serde_json::json!({
        "event":"native-display-software-offer", "host_id":view.host(), "boot_id":view.boot(),
        "offer_generation":view.generation(), "source_document_id":view.source(), "checked_plot_id":view.checked(), "expanded_plot_id":view.expanded(),
        "capabilities_bytes":serde_json::to_vec(&capabilities).unwrap().len(), "capabilities":capabilities,
        "authority_grants":[grant],"capability_possession_issued":false,"body_installed":false,"native_plan_installed":false,
    });
    let bytes = serde_json::to_vec(&reply).unwrap();
    let (observed, grants) = source.observe_native_offers(&host, &bytes, &host).unwrap();
    assert_eq!(observed.capabilities.len(), 4);
    PreparedDisplayOutput::prepare(
        &source,
        &observed,
        &["fixture/base".into(), "conduit.base/local@1".into()],
        &grants,
    )
    .unwrap();
    let mut changed = host.clone();
    changed.offer_generation.0 += 1;
    assert!(matches!(
        source.observe_native_offers(&host, &bytes, &changed),
        Err(E::NativeChanged)
    ));
    for (field, value, expected) in [
        ("boot_id", serde_json::json!("foreign"), E::Scope),
        ("expanded_plot_id", serde_json::json!("foreign"), E::Source),
        ("body_installed", serde_json::json!(true), E::Installed),
        (
            "capability_possession_issued",
            serde_json::json!(true),
            E::Authority,
        ),
        ("capabilities_bytes", serde_json::json!(0), E::Software),
    ] {
        let mut bad = reply.clone();
        bad[field] = value;
        assert_eq!(
            source
                .observe_native_offers(&host, &serde_json::to_vec(&bad).unwrap(), &host)
                .err(),
            Some(expected)
        );
    }
    for target in ["capability_id", "boot_id", "contract_id"] {
        let mut bad = reply.clone();
        bad["authority_grants"][0][target] = serde_json::json!("foreign");
        assert!(matches!(
            source.observe_native_offers(&host, &serde_json::to_vec(&bad).unwrap(), &host),
            Err(E::Authority)
        ));
    }
    let mut bad = reply.clone();
    bad["capabilities"][0]["artifact_id"] = serde_json::json!("foreign");
    assert!(source
        .observe_native_offers(&host, &serde_json::to_vec(&bad).unwrap(), &host)
        .is_err());
    let mut duplicate = host.clone();
    duplicate.capabilities.push(capabilities[0].clone());
    assert!(matches!(
        source.observe_native_offers(&duplicate, &bytes, &duplicate),
        Err(E::Software)
    ));
}
