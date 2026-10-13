//! Selection/admission fixture. Cloned signal plumbing makes no numeric execution claim.
use super::*;
use conduit_core::{
    ConfigurationEntry, ConfigurationValue, ExactRational as Q, KindConfigurationField,
    KindConfigurationRule, MathematicalValue as Value, NumericalAlgorithm as Algorithm,
    StructuredConfigurationValue, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoValue,
};
fn math(s: &str) -> Value {
    Value::parse_literal(s, &mut ExactNumericWork::new(VERIFICATION_WORK)).unwrap()
}
fn literal(value: Value) -> StructuredInfoValue {
    let ty = StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::MATHEMATICAL_VALUE_ID))
        .unwrap();
    let mut bytes = vec![0; conduit_core::MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let length = value.encode(&mut bytes).unwrap();
    bytes.truncate(length);
    StructuredInfoValue::leaf(ty, bytes).unwrap()
}
fn request(error: &str) -> StructuredConfigurationValue {
    let leaf = StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::MATHEMATICAL_VALUE_ID))
        .unwrap();
    let ty = StructuredInfoType::record(
        conduit_core::kind_id("fixture/numeric-request@1"),
        vec![
            StructuredFieldType::new("value", leaf.clone()).unwrap(),
            StructuredFieldType::new("maximum-error", leaf).unwrap(),
        ],
    )
    .unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let value = StructuredInfoValue::record(
        ty,
        vec![
            StructuredFieldValue::new("value", literal(math("0.3"))).unwrap(),
            StructuredFieldValue::new("maximum-error", literal(math(error))).unwrap(),
        ],
    )
    .unwrap();
    StructuredConfigurationValue::new(profile, value.canonical_bytes().unwrap()).unwrap()
}
fn fixture(error: &str) -> (conduit_plot::CheckedPlot, Vec<HostAdvertisement>) {
    let mut plot = crate::tests::plot();
    let mut host = crate::tests::host();
    let request_value_initial = request(error);
    let gear = &mut plot.gears[0];
    gear.semantic_contract
        .configuration
        .push(KindConfigurationField {
            key: "numeric-request".into(),
            default_value: ConfigurationValue::Structured(request_value_initial.clone()),
            rule: KindConfigurationRule::Structured {
                profile: request_value_initial.profile().clone(),
            },
        });
    gear.semantic_contract
        .laws
        .push(KindSemanticLaw::RealizationRequirement {
            property_profile: conduit_core::numerical_realization_profile(),
            configuration_key: "numeric-request".into(),
        });
    gear.configuration.push(ConfigurationEntry {
        key: "numeric-request".into(),
        value: ConfigurationValue::Structured(request_value_initial),
    });
    let request_value = request(error);
    gear.startup_parameters
        .push(conduit_core::FrontStartupParameter {
            name: "numeric-request".into(),
            value_type: request_value.profile().clone(),
            has_default: false,
        });
    host.capabilities[0].startup_parameters = gear.startup_parameters.clone();
    host.capabilities[0].semantic_contract = gear.semantic_contract.clone();
    let native_request =
        StructuredInfoValue::from_canonical_bytes(request_value.canonical_value()).unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    startup
        .insert_structured_type("NumericalRequest", native_request.value_type().clone())
        .unwrap();
    let existing = conduit_signal::signal_startup_catalog();
    let mut signature = existing
        .signature(conduit_signal::PULSE_KIND)
        .unwrap()
        .clone();
    signature
        .startup_parameters
        .push(conduit_plot::StartupParameterSignature {
            name: "numeric-request".into(),
            value_type: "NumericalRequest".into(),
            default: None,
        });
    startup.insert(signature).unwrap();
    startup
        .insert(
            existing
                .signature(conduit_signal::SHOW_KIND)
                .unwrap()
                .clone(),
        )
        .unwrap();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    let mut contract = conduit_signal::pulse_semantic_contract();
    contract.configuration = gear.semantic_contract.configuration.clone();
    contract.semantic_laws = gear.semantic_contract.laws.clone();
    contract.startup_parameters = gear.startup_parameters.clone();
    profiles.insert_kind(contract).unwrap();
    profiles
        .insert_kind(conduit_signal::show_semantic_contract())
        .unwrap();
    let source = format!("plot numeric-selection-fixture {{\n pulse: flow/pulse(count=2,period-ms=0,initial=false,numeric-request={{value:0.3,maximum-error:{error}}})\n show: presentation/show\n pulse >> show\n}}\n");
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profiles).unwrap();
    plot.validate_identities().unwrap();
    let mut other = host.clone();
    host.capabilities[0].limits.max_queue_items = 8;
    other.host_id = conduit_core::HostId::from("fixture/numeric-exact");
    for (h, algorithm) in [
        (&mut host, Algorithm::OutwardI64),
        (&mut other, Algorithm::ExactRational),
    ] {
        let prepared = PreparedNumericalRealization::prepare(
            &math("0.3"),
            algorithm,
            Q::ONE,
            &mut ExactNumericWork::new(VERIFICATION_WORK),
        )
        .unwrap();
        h.capabilities[0].realization_properties = vec![prepared.property().unwrap()];
    }
    (plot, vec![host, other])
}
#[test]
fn precision_gates_the_ordinary_selector_before_policy_and_is_sealed_in_plan() {
    use crate::{
        select_realization_with_policy, HardRealizationRequirements, PlacementChoices,
        RealizationPolicy,
    };
    let (plot, hosts) = fixture("0");
    let preference = RealizationPolicy {
        preferences: vec![crate::RealizationPreference::MaximizeQueueItems],
    };
    let choice = select_realization_with_policy(
        &plot.gears[0],
        &hosts,
        &HardRealizationRequirements::default(),
        &preference,
    )
    .unwrap();
    assert_eq!(choice.host_id.as_str(), "fixture/numeric-exact");
    let (loose_plot, loose_hosts) = fixture("1");
    let loose = select_realization_with_policy(
        &loose_plot.gears[0],
        &loose_hosts,
        &HardRealizationRequirements::default(),
        &preference,
    )
    .unwrap();
    assert_eq!(loose.host_id.as_str(), loose_hosts[0].host_id.as_str());

    let mut placements = alloc::collections::BTreeMap::new();
    placements.insert(plot.gears[0].gear_id.clone(), choice);
    // Preserve other fixture placements; they have no numeric requirements.
    for gear in plot.gears.iter().skip(1) {
        placements.insert(
            gear.gear_id.clone(),
            crate::PlacementChoice {
                host_id: hosts[1].host_id.clone(),
                capability_id: hosts[1]
                    .capabilities
                    .iter()
                    .find(|offer| gear.accepts_realization(offer))
                    .unwrap()
                    .capability_id
                    .clone(),
            },
        );
    }
    let plan = crate::plan(
        &plot,
        &hosts,
        &PlacementChoices {
            by_gear: placements,
        },
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
    )
    .unwrap();
    let placed = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|g| g.gear_id == plot.gears[0].gear_id)
        .unwrap();
    assert_eq!(
        placed.realization_properties,
        hosts[1].capabilities[0].realization_properties
    );
    assert!(conduit_core::verify_plan(&plan));
    let mut changed = plan;
    changed.fragments[0].placements[0]
        .realization_properties
        .clear();
    assert!(!conduit_core::verify_plan(&changed));
}
#[test]
fn direct_placement_cannot_bypass_a_numeric_requirement() {
    let (plot, hosts) = fixture("0");
    assert!(candidate_check(&plot.gears[0], &hosts[0].capabilities[0])
        .unwrap()
        .is_err());
    let (plot, hosts) = fixture("1");
    assert_eq!(
        candidate_check(&plot.gears[0], &hosts[0].capabilities[0]).unwrap(),
        Ok(())
    );
    let mut bad = hosts[0].clone();
    bad.capabilities[0].realization_properties.clear();
    assert!(candidate_check(&plot.gears[0], &bad.capabilities[0])
        .unwrap()
        .is_err());
}
