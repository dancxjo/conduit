use std::collections::BTreeMap;

use conduit_ai::{
    CPU_EXECUTION_RESOURCE, DATA_EGRESS_CHARACTERISTIC, HOST_MEMORY_GIB_RESOURCE,
    MAXIMUM_CONTEXT_CHARACTERISTIC,
};
use conduit_core::{
    ArtifactId, CapabilityId, CharacteristicId, CharacteristicUnit, ComputeServiceGuarantee,
    ComputeTopologyGroup, ComputeTopologyGroupId, HostId, ImplementationId,
    RealizationAdvertisement,
};
use conduit_planner::{
    dos_shell_style, plan, presentation_style_characteristics,
    select_realization_with_scoped_policy, HardRealizationRequirements, NamedStyle,
    PlacementChoice, PlacementChoices, PlannerFactRef, PlannerFactValue, PlannerPredicate,
    PlannerPreference, PolicyLayer, PolicyScope, PolicySourceId, PolicySourceRevision,
    PresentationStyleFacts, RealizationDecisionDisposition, RealizationPreference,
    ReviewedObservation, StyleId, PRESENTATION_DENSITY, PRESENTATION_FRAMING,
    PRESENTATION_KEYBOARD_VISIBLE, PRESENTATION_PALETTE_CLASS, PRESENTATION_TEXT_LAYOUT,
};

mod common;
use common::{generic_policy_facts, quantity, resource_observations};

fn source(id: &str, revision: u64, scope: PolicyScope) -> PolicySourceRevision {
    PolicySourceRevision {
        source_id: PolicySourceId::from(id),
        revision,
        scope,
    }
}

fn reviewed(
    hosts: &[conduit_core::HostAdvertisement],
    observed_epoch: u64,
    valid_through_epoch: u64,
) -> Vec<ReviewedObservation> {
    resource_observations(hosts)
        .into_iter()
        .map(|observation| ReviewedObservation {
            observation,
            source: source("reviewed-resource-monitor", 3, PolicyScope::SiteDeployment),
            observed_epoch,
            valid_through_epoch,
        })
        .collect()
}

fn soft_layer(id: &str, scope: PolicyScope, preferences: Vec<PlannerPreference>) -> PolicyLayer {
    PolicyLayer {
        source: source(id, 1, scope),
        hard_predicates: Vec::new(),
        preferences: preferences
            .into_iter()
            .map(RealizationPreference::Fact)
            .collect(),
    }
}

fn selected_plan(
    form: &conduit_form::CheckedForm,
    hosts: &[conduit_core::HostAdvertisement],
    choice: &PlacementChoice,
) -> conduit_core::Plan {
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([(form.gears[0].gear_id.clone(), choice.clone())]),
    };
    plan(form, hosts, &placements, &[]).expect("the exact selected realization seals into a Plan")
}

#[test]
fn llm_and_compute_policy_share_one_selector_and_produce_exact_replacement_plans() {
    let (form, mut hosts, advertisements) = generic_policy_facts();
    let cpu_id = conduit_core::ResourceClassId::from(CPU_EXECUTION_RESOURCE);
    let small_cpu = hosts
        .iter_mut()
        .find(|host| host.host_id.as_str() == "ai-small-local")
        .and_then(|host| {
            host.resources
                .iter_mut()
                .find(|pool| pool.class_id == cpu_id)
        })
        .expect("small local CPU fact exists");
    small_cpu
        .compute
        .as_mut()
        .expect("CPU is typed compute")
        .topology_groups = vec![ComputeTopologyGroup {
        group_id: ComputeTopologyGroupId::from("cluster-performance"),
        lane_capacity: 1,
        numa_domain: None,
        cache_domain: None,
        performance_class: None,
        nominal_clock_hz: Some(900_000_000),
    }];
    let observations = reviewed(&hosts, 8, 9);
    let semantic = source("checked-form", 1, PolicyScope::SemanticRequirements);

    let private_llm = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements {
            predicates: vec![
                PlannerPredicate::AtLeast {
                    fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                        MAXIMUM_CONTEXT_CHARACTERISTIC,
                    )),
                    value: quantity(24_000, CharacteristicUnit::Tokens),
                },
                PlannerPredicate::Equal {
                    fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                        DATA_EGRESS_CHARACTERISTIC,
                    )),
                    value: PlannerFactValue::Boolean(false),
                },
            ],
            ..HardRealizationRequirements::default()
        },
        semantic.clone(),
        &[soft_layer(
            "prefer-remote-context",
            PolicyScope::UserWorkspace,
            vec![PlannerPreference::Maximize {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    MAXIMUM_CONTEXT_CHARACTERISTIC,
                )),
            }],
        )],
        &observations,
        8,
    )
    .expect("hard privacy and context requirements dominate the remote favorite");
    assert_eq!(
        private_llm.selection.choice.host_id.as_str(),
        "ai-large-local"
    );
    assert!(private_llm.selection.signs.iter().any(|record| {
        record.host_id.as_str() == "ai-remote-base"
            && matches!(
                record.disposition,
                RealizationDecisionDisposition::Rejected(_)
            )
    }));

    let economical_compute = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts[..2],
        &advertisements[..2],
        &HardRealizationRequirements::default(),
        semantic,
        &[soft_layer(
            "minimize-host-memory",
            PolicyScope::SiteDeployment,
            vec![PlannerPreference::Minimize {
                fact: PlannerFactRef::ResourceUnits(conduit_core::ResourceClassId::from(
                    HOST_MEMORY_GIB_RESOURCE,
                )),
            }],
        )],
        &reviewed(&hosts[..2], 8, 9),
        8,
    )
    .expect("resource units are ranked as resource facts");
    assert_eq!(
        economical_compute.selection.choice.host_id.as_str(),
        "ai-small-local"
    );

    let performance_compute = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts[..2],
        &advertisements[..2],
        &HardRealizationRequirements {
            predicates: vec![PlannerPredicate::AtLeast {
                fact: PlannerFactRef::ComputeServiceGuarantee(cpu_id.clone()),
                value: PlannerFactValue::ServiceGuarantee(ComputeServiceGuarantee::Reserved),
            }],
            ..HardRealizationRequirements::default()
        },
        source("reserved-service", 1, PolicyScope::SemanticRequirements),
        &[soft_layer(
            "prefer-nominal-clock",
            PolicyScope::SiteDeployment,
            vec![PlannerPreference::Maximize {
                fact: PlannerFactRef::ComputeNominalClockHz {
                    resource_class_id: cpu_id,
                    topology_group_id: ComputeTopologyGroupId::from("cluster-performance"),
                },
            }],
        )],
        &reviewed(&hosts[..2], 8, 9),
        8,
    )
    .expect("service and topology remain typed compute facts");
    assert_eq!(
        performance_compute.selection.choice.host_id.as_str(),
        "ai-large-local"
    );

    let old_plan = selected_plan(&form, &hosts, &economical_compute.selection.choice);
    let replacement = selected_plan(&form, &hosts, &performance_compute.selection.choice);
    assert_ne!(old_plan.plan_id, replacement.plan_id);
    assert_eq!(
        old_plan.plan_id,
        selected_plan(&form, &hosts, &economical_compute.selection.choice).plan_id,
        "selecting a replacement does not mutate the old Plan"
    );
    for selection in [&private_llm, &economical_compute, &performance_compute] {
        assert!(selection
            .selection
            .signs
            .iter()
            .any(|record| { record.disposition == RealizationDecisionDisposition::Selected }));
        assert!(!selection.basis.policy_sources.is_empty());
        assert!(!selection.basis.observations.is_empty());
    }
}

fn presentation_fixture() -> (
    conduit_form::CheckedForm,
    Vec<conduit_core::HostAdvertisement>,
    Vec<RealizationAdvertisement>,
) {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_patchbay_presentation_catalogs(&mut startup, &mut profile)
        .expect("presentation catalogs install");
    let form = conduit_form::parse(
        "form styled {\n    canvas: presentation/patchbay\n}\n",
        &profile,
    )
    .expect("semantic presentation checks");
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
    for (family, facts) in profiles {
        let mut host = conduit_std_catalog::standard_host_advertisement(
            HostId::from(format!("style-{family}")),
            conduit_core::BootId::from(format!("style-{family}-boot")),
            conduit_core::OfferGeneration(1),
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
            capability_id: offer.capability_id,
            characteristics: presentation_style_characteristics(&facts),
        });
        hosts.push(host);
    }
    (form, hosts, advertisements)
}

#[test]
fn style_is_another_policy_layer_not_a_presenter_specific_selector() {
    let (form, hosts, advertisements) = presentation_fixture();
    let observations = reviewed(&hosts, 5, 5);
    let accessibility = HardRealizationRequirements {
        predicates: vec![PlannerPredicate::Equal {
            fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                PRESENTATION_KEYBOARD_VISIBLE,
            )),
            value: PlannerFactValue::Boolean(true),
        }],
        ..HardRealizationRequirements::default()
    };
    let spacious = NamedStyle {
        style_id: StyleId::from("conduit.style/spacious@1"),
        revision: 1,
        preferences: vec![
            style_equal(PRESENTATION_TEXT_LAYOUT, "flow"),
            style_equal(PRESENTATION_DENSITY, "comfortable"),
            style_equal(PRESENTATION_FRAMING, "minimal"),
            style_equal(PRESENTATION_PALETTE_CLASS, "system-adaptive"),
        ],
    };
    let semantic = source(
        "accessible-presentation",
        1,
        PolicyScope::SemanticRequirements,
    );
    let dos = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &accessibility,
        semantic.clone(),
        &[dos_shell_style().lower().expect("reviewed STYLE lowers")],
        &observations,
        5,
    )
    .expect("DOS STYLE flows through the generic selector");
    let spacious = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &accessibility,
        semantic,
        &[spacious.lower().expect("reviewed STYLE lowers")],
        &observations,
        5,
    )
    .expect("spacious STYLE flows through the same generic selector");
    assert_eq!(dos.selection.choice.host_id.as_str(), "style-native");
    assert_eq!(spacious.selection.choice.host_id.as_str(), "style-browser");
    assert_ne!(
        selected_plan(&form, &hosts, &dos.selection.choice).plan_id,
        selected_plan(&form, &hosts, &spacious.selection.choice).plan_id
    );
    assert!(dos
        .basis
        .observations
        .iter()
        .all(|basis| basis.observed_epoch == 5));
    assert!(spacious.selection.signs.iter().any(|record| {
        record.disposition == RealizationDecisionDisposition::Selected
            && record.decisive_preference_source.as_ref()
                == Some(&source(
                    "conduit.style/spacious@1",
                    1,
                    PolicyScope::NamedStyle,
                ))
    }));
}

fn style_equal(id: &str, value: &str) -> PlannerPreference {
    PlannerPreference::PreferEqual {
        fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(id)),
        value: PlannerFactValue::Category(value.into()),
    }
}

#[test]
fn generic_negative_boundaries_hold_across_domain_vocabulary() {
    let (form, hosts, mut advertisements) = generic_policy_facts();
    let stale = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        source("checked-form", 1, PolicyScope::SemanticRequirements),
        &[],
        &reviewed(&hosts, 2, 2),
        3,
    )
    .expect_err("stale observations cannot participate in a new planning basis");
    assert!(matches!(
        stale,
        conduit_planner::PlannerError::CurrentResourceObservationUnavailable(_)
    ));

    let unordered = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        source("checked-form", 1, PolicyScope::SemanticRequirements),
        &[soft_layer(
            "invalid-category-magnitude",
            PolicyScope::SiteDeployment,
            vec![PlannerPreference::Maximize {
                fact: PlannerFactRef::ComputePerformanceClass {
                    resource_class_id: conduit_core::ResourceClassId::from(CPU_EXECUTION_RESOURCE),
                    topology_group_id: ComputeTopologyGroupId::from("cluster-performance"),
                },
            }],
        )],
        &reviewed(&hosts, 3, 3),
        3,
    )
    .expect_err("unordered categories cannot become a magnitude or universal score");
    assert!(matches!(
        unordered,
        conduit_planner::PlannerError::InvalidRealizationPolicy(_)
    ));

    advertisements[0]
        .characteristics
        .push(conduit_core::RealizationCharacteristic {
            definition: conduit_core::CharacteristicDefinition {
                characteristic_id: CharacteristicId::from("resource/cpu-count"),
                subject: conduit_core::CharacteristicSubject::Resource,
                stability: conduit_core::CharacteristicStability::Stable,
                value_kind: conduit_core::CharacteristicValueKind::UnsignedQuantity {
                    unit: CharacteristicUnit::Items,
                    maximum: 64,
                },
                human_name: "CPU count".into(),
                help: "Intentionally invalid cross-subject fixture.".into(),
            },
            value: conduit_core::CharacteristicValue::UnsignedQuantity {
                value: 3,
                unit: CharacteristicUnit::Items,
            },
        });
    let wrong_subject = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        source("checked-form", 1, PolicyScope::SemanticRequirements),
        &[],
        &reviewed(&hosts, 3, 3),
        3,
    )
    .expect_err("a resource fact cannot masquerade as a realization characteristic");
    assert!(matches!(
        wrong_subject,
        conduit_planner::PlannerError::InvalidHardRealizationRequirement(_)
    ));
}
