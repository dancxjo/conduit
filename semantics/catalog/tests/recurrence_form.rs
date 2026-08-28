use conduit_core::{
    CapabilityOffer, ConfigurationValue, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    StructuredInfoValue, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

pub const CIVIL_FORM: &str = r#"form meeting {
  expand: time/expand-recurrence({ excluded_ordinals: [unused(""), unused(""), unused(""), unused("")], fold_policy: "earlier", gap_policy: "skip", identity: "recurrence/weekly-meeting", maximum_occurrences: 3, maximum_results: 2, resolutions: [unique({ instant: { basis: "utc", resolution_ticks: 1, scale: "seconds", ticks: 100 }, local_date: "2026-03-02", local_time: "09:00:00", ordinal: 0, rule_set: "tzdb/2026a", zone: "America/Los_Angeles" }), unique({ instant: { basis: "utc", resolution_ticks: 1, scale: "seconds", ticks: 300 }, local_date: "2026-03-16", local_time: "09:00:00", ordinal: 2, rule_set: "tzdb/2026a", zone: "America/Los_Angeles" }), unused(""), unused(""), unused(""), unused(""), unused(""), unused("")], rule: civil_weekdays({ excluded_dates: [exclude("2026-03-09"), unused(""), unused(""), unused("")], first_date: "2026-03-02", local_time: "09:00:00", rule_set: "tzdb/2026a", weekdays: 1, zone: "America/Los_Angeles" }), until: civil_date("2026-03-16"), window: wall({ end: { basis: "utc", resolution_ticks: 1, scale: "seconds", ticks: 300 }, start: { basis: "utc", resolution_ticks: 1, scale: "seconds", ticks: 100 } }) })
}
"#;

#[test]
fn civil_recurrence_is_checked_and_planned_as_one_exact_typed_request() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_recurrence_catalogs(&mut startup, &mut profile).unwrap();
    let parsed = parse_syntax_document(CIVIL_FORM);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "meeting", &profile).unwrap();
    let offer = common::recurrence_proof_offer();
    let host = host(offer);
    let placements =
        conduit_planner::default_expanded_placements(&expanded, core::slice::from_ref(&host))
            .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        core::slice::from_ref(&host),
        &placements,
        &[],
    )
    .unwrap();
    let ConfigurationValue::Structured(request) =
        &plan.fragments[0].placements[0].configuration[0].value
    else {
        panic!("planned recurrence request must remain structured Info")
    };
    let decoded = StructuredInfoValue::from_canonical_bytes(request.canonical_value()).unwrap();
    assert_eq!(
        decoded.value_type(),
        &conduit_semantic_catalog::recurrence_request_type()
    );
}

fn host(offer: CapabilityOffer) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/recurrence"),
        boot_id: conduit_core::BootId::from("boot/recurrence"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/recurrence-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![offer],
    }
}
mod common;
