//! Named syntax-selection fixture: cloned finite signal plumbing carries no parser claim.
use super::*;
use conduit_core::{
    ConfigurationEntry, KindConfigurationField, KindConfigurationRule, KindSemanticLaw,
};
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;

fn language(name: &str) -> LanguageId {
    LanguageId::new(name.into()).unwrap()
}
fn request(name: &str, variety: Option<LanguageVariety>, exact: bool) -> LanguageRequest {
    LanguageRequest::new(
        language(name),
        variety,
        if exact {
            LanguageVarietyPolicy::ExactVariety
        } else {
            LanguageVarietyPolicy::LanguageSufficient
        },
    )
    .unwrap()
}
fn coverage(names: &[&str], varieties: Vec<LanguageVariety>, sensitive: bool) -> LanguageCoverage {
    LanguageCoverage::new(
        "fixture/declared-not-parser-accuracy".into(),
        BoundedSequence::try_from_iter(names.iter().map(|n| language(n))).unwrap(),
        BoundedSequence::try_from_iter([]).unwrap(),
        "fixture@1".into(),
        BoundedSequence::try_from_iter(varieties).unwrap(),
        sensitive,
    )
    .unwrap()
}
fn fixture(request: LanguageRequest) -> (CheckedPlot, HostAdvertisement) {
    let mut plot = crate::tests::plot();
    let mut host = crate::tests::host();
    let variety_source = request
        .variety()
        .as_ref()
        .map(|v| {
            format!(
                "some({{ identity: {:?}, language: {:?} }})",
                v.identity().get(),
                v.language().get()
            )
        })
        .unwrap_or_else(|| "none(empty)".into());
    let policy_source = if *request.variety_policy() == LanguageVarietyPolicy::ExactVariety {
        "exact_variety(empty)"
    } else {
        "language_sufficient(empty)"
    };
    let request_source = format!(
        "{{ language: {:?}, variety: {}, variety_policy: {} }}",
        request.language().get(),
        variety_source,
        policy_source
    );
    let value = language_request_configuration(request).unwrap();
    let gear = &mut plot.gears[0];
    gear.semantic_contract
        .configuration
        .push(KindConfigurationField {
            key: "language-request".into(),
            default_value: value.clone(),
            rule: KindConfigurationRule::Structured {
                profile: language_request_profile(),
            },
        });
    gear.semantic_contract
        .laws
        .push(KindSemanticLaw::RealizationRequirement {
            property_profile: language_coverage_profile(),
            configuration_key: "language-request".into(),
        });
    gear.configuration.push(ConfigurationEntry {
        key: "language-request".into(),
        value,
    });
    gear.startup_parameters
        .push(conduit_core::FrontStartupParameter {
            name: "language-request".into(),
            value_type: language_request_profile(),
            has_default: false,
        });
    host.capabilities[0].startup_parameters = gear.startup_parameters.clone();
    host.capabilities[0].semantic_contract = gear.semantic_contract.clone();
    let mut startup = conduit_plot::StartupCatalog::new();
    startup
        .insert_structured_type("LanguageRequest", LanguageRequest::semantic_type().unwrap())
        .unwrap();
    let existing = conduit_signal::signal_startup_catalog();
    let mut signature = existing
        .signature(conduit_signal::PULSE_KIND)
        .unwrap()
        .clone();
    signature
        .startup_parameters
        .push(conduit_plot::StartupParameterSignature {
            name: "language-request".into(),
            value_type: "LanguageRequest".into(),
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
    let mut profile = conduit_plot::ProfileCatalog::new();
    let mut contract = conduit_signal::pulse_semantic_contract();
    contract.configuration = gear.semantic_contract.configuration.clone();
    contract.semantic_laws = gear.semantic_contract.laws.clone();
    contract.startup_parameters = gear.startup_parameters.clone();
    profile.insert_kind(contract).unwrap();
    profile
        .insert_kind(conduit_signal::show_semantic_contract())
        .unwrap();
    let source=format!("plot signal-demo {{\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false, language-request = {request_source})\n show: presentation/show\n pulse >> show\n}}\n");
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profile).unwrap();
    plot.validate_identities().unwrap();
    (plot, host)
}
fn declare(
    host: &mut HostAdvertisement,
    names: &[&str],
    varieties: Vec<LanguageVariety>,
    sensitive: bool,
) {
    host.capabilities[0].realization_properties =
        vec![language_coverage_property(coverage(names, varieties, sensitive)).unwrap()];
}
fn refusal(error: PlannerError, expected: &str, reason: LanguageCoverageRefusal) {
    let PlannerError::LanguageCoverageUnsatisfied(evidence) = error else {
        panic!("unexpected refusal {error:?}")
    };
    let crate::LanguageCoverageUnsatisfied {
        requirements,
        candidates,
        ..
    } = *evidence;
    assert_eq!(requirements[0].request.language().get(), expected);
    assert_eq!(candidates[0].checks[0].result, Err(reason));
}
#[test]
fn fore_equality_cannot_admit_french_or_undeclared_back_in_any_ordinary_entrance() {
    let (plot, mut host) = fixture(request("French", None, false));
    assert!(plot.gears[0].accepts_realization(&host.capabilities[0]));
    for declaration in [
        None,
        Some(coverage(&[], vec![], false)),
        Some(coverage(&["English"], vec![], false)),
    ] {
        host.capabilities[0].realization_properties = declaration
            .map(|c| vec![language_coverage_property(c).unwrap()])
            .unwrap_or_default();
        let reason = if declaration_is_english(&host) {
            LanguageCoverageRefusal::Language
        } else {
            LanguageCoverageRefusal::Undeclared
        };
        refusal(
            default_placements(&plot, core::slice::from_ref(&host)).unwrap_err(),
            "French",
            reason,
        );
        refusal(
            select_realization_with_policy(
                &plot.gears[0],
                core::slice::from_ref(&host),
                &HardRealizationRequirements::default(),
                &RealizationPolicy::default(),
            )
            .unwrap_err(),
            "French",
            reason,
        );
        refusal(
            select_realization_with_characteristics(
                &plot.gears[0],
                core::slice::from_ref(&host),
                &[],
                &HardRealizationRequirements::default(),
                &[],
                &RealizationPolicy::default(),
            )
            .unwrap_err(),
            "French",
            reason,
        );
        let mut placements = PlacementChoices {
            by_gear: BTreeMap::new(),
        };
        for (gear, offer) in plot.gears.iter().zip(&host.capabilities) {
            placements.by_gear.insert(
                gear.gear_id.clone(),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: offer.capability_id.clone(),
                },
            );
        }
        refusal(
            plan(
                &plot,
                core::slice::from_ref(&host),
                &placements,
                &[BaseImplementationId::from("conduit.base/local@1")],
            )
            .unwrap_err(),
            "French",
            reason,
        );
    }
}
fn declaration_is_english(host: &HostAdvertisement) -> bool {
    inspect_language_coverage(
        &fixture(request("English", None, false)).0.gears[0],
        core::slice::from_ref(host),
    )
    .unwrap()[0]
        .checks[0]
        .result
        .is_ok()
}
#[test]
fn multilingual_and_narrow_backs_share_kind_and_policy_runs_after_coverage() {
    let (mut plot, mut host) = fixture(request("English", None, false));
    declare(&mut host, &["English"], vec![], false);
    host.capabilities[0].limits.max_queue_items = 100;
    let mut broad = host.capabilities[0].clone();
    broad.capability_id = CapabilityId::from("multilingual");
    broad.limits.max_queue_items = 10;
    broad.realization_properties =
        vec![language_coverage_property(coverage(&["English", "French"], vec![], false)).unwrap()];
    host.capabilities.push(broad);
    let policy = RealizationPolicy {
        preferences: vec![RealizationPreference::MaximizeQueueItems],
    };
    let choose = |gear: &CheckedGear| {
        select_realization_with_policy(
            gear,
            core::slice::from_ref(&host),
            &HardRealizationRequirements::default(),
            &policy,
        )
        .unwrap()
    };
    assert_eq!(
        choose(&plot.gears[0]).capability_id,
        host.capabilities[0].capability_id
    );
    plot.gears[0].configuration.last_mut().unwrap().value =
        language_request_configuration(request("French", None, false)).unwrap();
    assert_eq!(
        choose(&plot.gears[0]).capability_id,
        CapabilityId::from("multilingual")
    );
    let mut observation_free = host.clone();
    for offer in &mut observation_free.capabilities {
        offer.resource_requirements.clear();
    }
    let selected = select_realization_with_characteristics_and_signs(
        &plot.gears[0],
        &[observation_free],
        &[],
        &HardRealizationRequirements::default(),
        &[],
        &policy,
    )
    .unwrap();
    assert_eq!(
        selected.choice.capability_id,
        CapabilityId::from("multilingual")
    );
    assert!(selected.signs.iter().any(|record| matches!(&record.disposition, RealizationDecisionDisposition::Rejected(RealizationRejection::LanguageCoverage { request,reason:LanguageCoverageRefusal::Language }) if request.language().get()=="French")));
    let evidence = inspect_language_coverage(&plot.gears[0], &[host]).unwrap();
    assert_eq!(evidence.len(), 2);
    assert_eq!(
        evidence[0].checks[0].result,
        Err(LanguageCoverageRefusal::Language)
    );
    assert!(evidence[1].checks[0].result.is_ok());
}
#[test]
fn variety_requirement_and_back_sensitivity_are_separate_exact_facts() {
    let french = language("French");
    let requested =
        LanguageVariety::new(VarietyId::new("quebec".into()).unwrap(), french.clone()).unwrap();
    let supported = LanguageVariety::new(VarietyId::new("paris".into()).unwrap(), french).unwrap();
    for (semantic_exact, back_sensitive) in [(true, false), (false, true), (false, false)] {
        let (plot, mut host) = fixture(request("French", Some(requested.clone()), semantic_exact));
        declare(
            &mut host,
            &["French"],
            vec![supported.clone()],
            back_sensitive,
        );
        let details =
            inspect_language_coverage(&plot.gears[0], core::slice::from_ref(&host)).unwrap();
        assert_eq!(
            details[0].checks[0].requirement.request.variety(),
            &Some(requested.clone())
        );
        if semantic_exact || back_sensitive {
            refusal(
                default_placements(&plot, &[host]).unwrap_err(),
                "French",
                LanguageCoverageRefusal::Variety,
            );
        } else {
            assert!(default_placements(&plot, &[host]).is_ok());
        }
    }
}

#[test]
fn selected_coverage_is_retained_and_cannot_change_under_the_same_plan_identity() {
    let (plot, mut host) = fixture(request("English", None, false));
    declare(&mut host, &["English"], vec![], false);
    let placements = default_placements(&plot, core::slice::from_ref(&host)).unwrap();
    let original = plan(
        &plot,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    assert_eq!(
        original.fragments[0].placements[0].realization_properties,
        host.capabilities[0].realization_properties
    );
    let mut changed = original.clone();
    changed.fragments[0].placements[0]
        .realization_properties
        .clear();
    assert!(!conduit_core::verify_plan(&changed));
    declare(&mut host, &["English", "French"], vec![], false);
    let next = plan(
        &plot,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    assert_ne!(original.plan_id, next.plan_id);
}
