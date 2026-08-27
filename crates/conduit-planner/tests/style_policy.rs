use conduit_core::{
    ArtifactId, CapabilityId, CharacteristicId, HostId, ImplementationId, RealizationAdvertisement,
};
use conduit_planner::{
    dos_shell_style, presentation_style_characteristics, select_realization_with_style,
    HardRealizationRequirements, NamedStyle, PlannerError, PlannerFactRef, PlannerFactValue,
    PlannerPredicate, PlannerPreference, PolicyScope, PolicySourceId, PolicySourceRevision,
    PresentationStyleFacts, ReviewedObservation, StyleId, StylePreferenceOutcome,
    PRESENTATION_KEYBOARD_VISIBLE, PRESENTATION_PALETTE_CLASS,
};

mod common;

fn presentation_fixture() -> (
    conduit_form::CheckedGear,
    Vec<conduit_core::HostAdvertisement>,
    Vec<RealizationAdvertisement>,
    Vec<ReviewedObservation>,
) {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_patchbay_presentation_catalogs(&mut startup, &mut profile)
        .expect("Patchbay presentation catalogs install");
    let gear = conduit_form::parse(
        "form styled {\n    canvas: presentation/patchbay\n}\n",
        &profile,
    )
    .expect("semantic Patchbay presentation checks")
    .gears
    .remove(0);

    let profiles = [
        (
            "native",
            PresentationStyleFacts {
                text_layout: Some("fixed-cell".into()),
                density: Some("compact".into()),
                framing: Some("hard-line".into()),
                palette_class: Some("phosphor-cyan-amber".into()),
                keyboard_visible: Some(true),
            },
        ),
        (
            "browser",
            PresentationStyleFacts {
                text_layout: Some("flow".into()),
                density: Some("comfortable".into()),
                framing: Some("minimal".into()),
                palette_class: Some("system-adaptive".into()),
                keyboard_visible: Some(true),
            },
        ),
        (
            "terminal",
            PresentationStyleFacts {
                text_layout: Some("fixed-cell".into()),
                density: Some("compact".into()),
                framing: Some("hard-line".into()),
                palette_class: None,
                keyboard_visible: Some(true),
            },
        ),
    ];
    let mut hosts = Vec::new();
    let mut advertisements = Vec::new();
    let mut observations = Vec::new();
    for (index, (family, facts)) in profiles.into_iter().enumerate() {
        let mut host = common::standard_planning_fixture(
            HostId::from(format!("style-{family}")),
            conduit_core::BootId::from(format!("style-{family}-boot")),
        );
        let mut offer = conduit_std_catalog::patchbay_presentation_offers()[0].clone();
        offer.capability_id = CapabilityId::from(format!("style-{family}/patchbay@1"));
        offer.implementation.implementation_id =
            ImplementationId::from(format!("style-{family}/presenter@1"));
        offer.implementation.artifact_id = ArtifactId::from(format!("style-{family}/artifact@1"));
        host.capabilities = vec![offer.clone()];
        advertisements.push(RealizationAdvertisement {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            capability_id: offer.capability_id.clone(),
            characteristics: presentation_style_characteristics(&facts),
        });
        for (pool_index, pool) in host.resources.iter().enumerate() {
            observations.push(ReviewedObservation {
                observation: conduit_core::ResourceObservation {
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    offer_generation: host.offer_generation,
                    pool_id: pool.pool_id.clone(),
                    class_id: pool.class_id.clone(),
                    health: conduit_core::ResourceHealth::Ready,
                    unreserved_units: pool.capacity_units,
                    utilized_units: 0,
                    sign_id: conduit_core::SignId::from(format!(
                        "style-{family}-{index}-{pool_index}"
                    )),
                },
                source: source("style-observer", 1, PolicyScope::SiteDeployment),
                observed_epoch: 4,
                valid_through_epoch: 4,
            });
        }
        hosts.push(host);
    }
    (gear, hosts, advertisements, observations)
}

fn source(id: &str, revision: u64, scope: PolicyScope) -> PolicySourceRevision {
    PolicySourceRevision {
        source_id: PolicySourceId::new(id),
        revision,
        scope,
    }
}

#[test]
fn one_named_style_lowers_through_c3_and_selects_truthful_host_specific_implementations() {
    let (gear, hosts, advertisements, observations) = presentation_fixture();
    let style = dos_shell_style();
    for expected in ["native", "browser", "terminal"] {
        let host = hosts
            .iter()
            .find(|host| host.host_id.as_str() == format!("style-{expected}"))
            .expect("presenter family exists");
        let host_observations = observations
            .iter()
            .filter(|item| item.observation.host_id == host.host_id)
            .cloned()
            .collect::<Vec<_>>();
        let host_advertisements = advertisements
            .iter()
            .filter(|item| item.host_id == host.host_id)
            .cloned()
            .collect::<Vec<_>>();
        let result = select_realization_with_style(
            &gear,
            core::slice::from_ref(host),
            &host_advertisements,
            &HardRealizationRequirements::default(),
            source(
                "semantic-presentation",
                1,
                PolicyScope::SemanticRequirements,
            ),
            &[],
            &style,
            &host_observations,
            4,
        )
        .expect("the same STYLE selects the truthful available presenter");
        assert_eq!(
            result.scoped.selection.choice.capability_id.as_str(),
            format!("style-{expected}/patchbay@1")
        );
    }
}

#[test]
fn partial_style_satisfaction_is_legal_and_inspectable() {
    let (gear, hosts, advertisements, observations) = presentation_fixture();
    let terminal = hosts
        .iter()
        .find(|host| host.host_id.as_str() == "style-terminal")
        .unwrap();
    let result = select_realization_with_style(
        &gear,
        core::slice::from_ref(terminal),
        &advertisements
            .iter()
            .filter(|item| item.host_id == terminal.host_id)
            .cloned()
            .collect::<Vec<_>>(),
        &HardRealizationRequirements::default(),
        source(
            "semantic-presentation",
            1,
            PolicyScope::SemanticRequirements,
        ),
        &[],
        &dos_shell_style(),
        &observations
            .iter()
            .filter(|item| item.observation.host_id == terminal.host_id)
            .cloned()
            .collect::<Vec<_>>(),
        4,
    )
    .expect("missing palette preference does not invalidate the presentation");
    assert_eq!(result.preferences.len(), 5);
    assert_eq!(
        result.preferences[4].outcome,
        StylePreferenceOutcome::Unavailable
    );
    assert!(result.preferences[..4]
        .iter()
        .all(|item| item.outcome == StylePreferenceOutcome::Matched));
}

#[test]
fn hard_accessibility_truth_outranks_a_conflicting_style() {
    let (gear, mut hosts, mut advertisements, observations) = presentation_fixture();
    let native_advertisement = advertisements
        .iter_mut()
        .find(|item| item.host_id.as_str() == "style-native")
        .unwrap();
    let keyboard = native_advertisement
        .characteristics
        .iter_mut()
        .find(|item| item.definition.characteristic_id.as_str() == PRESENTATION_KEYBOARD_VISIBLE)
        .unwrap();
    keyboard.value = conduit_core::CharacteristicValue::Boolean(false);
    hosts.retain(|host| matches!(host.host_id.as_str(), "style-native" | "style-terminal"));
    advertisements
        .retain(|item| matches!(item.host_id.as_str(), "style-native" | "style-terminal"));
    let observations = observations
        .into_iter()
        .filter(|item| {
            matches!(
                item.observation.host_id.as_str(),
                "style-native" | "style-terminal"
            )
        })
        .collect::<Vec<_>>();
    let hostile_style = NamedStyle {
        style_id: StyleId::from("conduit.style/hidden-keyboard@1"),
        revision: 1,
        preferences: vec![PlannerPreference::PreferEqual {
            fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                PRESENTATION_KEYBOARD_VISIBLE,
            )),
            value: PlannerFactValue::Boolean(false),
        }],
    };
    let result = select_realization_with_style(
        &gear,
        &hosts,
        &advertisements,
        &HardRealizationRequirements {
            predicates: vec![PlannerPredicate::Equal {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    PRESENTATION_KEYBOARD_VISIBLE,
                )),
                value: PlannerFactValue::Boolean(true),
            }],
            ..HardRealizationRequirements::default()
        },
        source("accessibility", 9, PolicyScope::SemanticRequirements),
        &[],
        &hostile_style,
        &observations,
        4,
    )
    .expect("hard keyboard visibility removes the STYLE favorite");
    assert_eq!(
        result.scoped.selection.choice.host_id.as_str(),
        "style-terminal"
    );
}

#[test]
fn style_rejects_renderer_specific_vocabulary_and_contains_no_hard_clauses() {
    let style = NamedStyle {
        style_id: StyleId::from("conduit.style/css-leak@1"),
        revision: 1,
        preferences: vec![PlannerPreference::PreferEqual {
            fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                "presentation/css-color",
            )),
            value: PlannerFactValue::Category("#00ffff".into()),
        }],
    };
    assert!(matches!(
        style.lower(),
        Err(PlannerError::InvalidRealizationPolicy(_))
    ));
    let lowered = dos_shell_style().lower().expect("reviewed STYLE lowers");
    assert!(lowered.hard_predicates.is_empty());
    assert_eq!(lowered.source.scope, PolicyScope::NamedStyle);
    assert_eq!(
        lowered.preferences[4].lower().fact(),
        &PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
            PRESENTATION_PALETTE_CLASS,
        ))
    );
}
