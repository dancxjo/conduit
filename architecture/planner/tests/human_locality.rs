use std::collections::BTreeMap;

use conduit_core::{
    verify_plan, BaseImplementationId, BootId, GearId, HostId, LineId, LinkBindingId,
    LinkEndpointId, ResourceClassId, ResourceHealth, ResourceObservation, SignId,
};
use conduit_planner::{
    plan_selected_realizations_with_characteristics_and_options,
    select_realization_with_characteristics_and_signs, select_realization_with_scoped_policy,
    HardRealizationRequirements, PlannerFactRef, PlannerFactValue, PlannerPredicate,
    PlannerPreference, PlanningOptions, PolicyLayer, PolicyScope, RealizationDecisionDisposition,
    RealizationPolicy, RealizationPreference,
};

mod common;

#[path = "human_locality/lifecycle.rs"]
mod lifecycle;

const LOCAL: &str = "host/zz-constrained-laptop";
const REMOTE: &str = "host/aa-capable-workstation";
const CPU: &str = conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS;

fn form() -> conduit_form::CheckedForm {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    conduit_form::parse(
        "form text_lab {\n keyboard: input/keyboard\n keymap: input/keymap\n uppercase: text/upper\n presentation: presentation/text\n keyboard.key > keymap.key\n keymap.text > uppercase.text\n uppercase.text > presentation.text\n}\n",
        &profile,
    )
    .expect("the unchanged text-lab Form checks")
}

fn hosts() -> Vec<conduit_core::HostAdvertisement> {
    let mut local =
        common::standard_planning_fixture(HostId::from(LOCAL), BootId::from("boot/constrained-1"));
    local
        .capabilities
        .push(conduit_std_offers::hosted_keyboard_offer(
            "window-keyboard-v1",
            "native-window-keyboard@1",
        ));
    local.resources.push(conduit_core::resource_offer(
        "constrained/heavy-work",
        CPU,
        16,
    ));
    local.resources.push(conduit_core::resource_offer(
        "constrained/window-input",
        conduit_core::INPUT_RESOURCE_CLASS,
        1,
    ));
    local
        .capabilities
        .iter_mut()
        .find(|offer| offer.kind_id.as_str() == "text/upper")
        .unwrap()
        .resource_requirements
        .push(conduit_core::resource_requirement(CPU, 8));
    local
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    local
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let mut remote =
        common::standard_planning_fixture(HostId::from(REMOTE), BootId::from("boot/workstation-1"));
    remote.resources.push(conduit_core::resource_offer(
        "workstation/heavy-work",
        CPU,
        16,
    ));
    remote
        .capabilities
        .iter_mut()
        .find(|offer| offer.kind_id.as_str() == "text/upper")
        .unwrap()
        .resource_requirements
        .push(conduit_core::resource_requirement(CPU, 8));
    remote
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    remote
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    vec![local, remote]
}

fn observations(hosts: &[conduit_core::HostAdvertisement]) -> Vec<ResourceObservation> {
    hosts
        .iter()
        .flat_map(|host| {
            host.resources.iter().enumerate().map(move |(index, pool)| {
                let unreserved_units = if pool.class_id.as_str() == CPU {
                    if host.host_id.as_str() == LOCAL {
                        1
                    } else {
                        16
                    }
                } else {
                    pool.capacity_units
                };
                ResourceObservation {
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    offer_generation: host.offer_generation,
                    pool_id: pool.pool_id.clone(),
                    class_id: pool.class_id.clone(),
                    health: ResourceHealth::Ready,
                    unreserved_units,
                    utilized_units: pool.capacity_units - unreserved_units,
                    sign_id: SignId::from(format!("resource/{}/{index}", host.host_id.as_str())),
                }
            })
        })
        .collect()
}

fn prefer_host(host: &str) -> RealizationPolicy {
    RealizationPolicy {
        preferences: vec![RealizationPreference::Fact(
            PlannerPreference::PreferEqual {
                fact: PlannerFactRef::HostIdentity,
                value: PlannerFactValue::Category(host.into()),
            },
        )],
    }
}

fn require_host(host: &str) -> PlannerPredicate {
    PlannerPredicate::Equal {
        fact: PlannerFactRef::HostIdentity,
        value: PlannerFactValue::Category(host.into()),
    }
}

fn lines(hosts: &[conduit_core::HostAdvertisement]) -> Vec<conduit_core::LineOffer> {
    let exact = conduit_signal_conformance::triple::exact_plan().expect("reviewed Line fixture");
    let mut outward = exact.browser_line;
    outward.line_id = LineId::from("line/laptop-to-workstation");
    outward.binding.binding_id = LinkBindingId::from("binding/laptop-to-workstation");
    outward.binding.source.host_id = hosts[0].host_id.clone();
    outward.binding.source.boot_id = hosts[0].boot_id.clone();
    outward.binding.source.endpoint_id = LinkEndpointId::from("endpoint/laptop-egress");
    outward.binding.sink.host_id = hosts[1].host_id.clone();
    outward.binding.sink.boot_id = hosts[1].boot_id.clone();
    outward.binding.sink.endpoint_id = LinkEndpointId::from("endpoint/workstation-ingress");
    outward.availability.line_id = outward.line_id.clone();
    outward.availability.binding_id = outward.binding.binding_id.clone();
    outward.binding.limits.maximum_in_flight_items = 4;
    outward.binding.limits.maximum_payload_bytes = 256;
    outward.binding.limits.maximum_frame_bytes = 512;
    outward.binding.limits.maximum_buffered_bytes = 1_024;

    let mut returning = outward.clone();
    returning.line_id = LineId::from("line/workstation-to-laptop");
    returning.binding.binding_id = LinkBindingId::from("binding/workstation-to-laptop");
    returning.binding.source.host_id = hosts[1].host_id.clone();
    returning.binding.source.boot_id = hosts[1].boot_id.clone();
    returning.binding.source.endpoint_id = LinkEndpointId::from("endpoint/workstation-egress");
    returning.binding.sink.host_id = hosts[0].host_id.clone();
    returning.binding.sink.boot_id = hosts[0].boot_id.clone();
    returning.binding.sink.endpoint_id = LinkEndpointId::from("endpoint/laptop-ingress");
    returning.availability.line_id = returning.line_id.clone();
    returning.availability.binding_id = returning.binding.binding_id.clone();
    vec![outward, returning]
}

fn requirements() -> BTreeMap<GearId, HardRealizationRequirements> {
    BTreeMap::from([(
        GearId::from("text_lab/uppercase"),
        HardRealizationRequirements {
            predicates: vec![PlannerPredicate::AtLeast {
                fact: PlannerFactRef::ObservationUnreservedUnits(ResourceClassId::from(CPU)),
                value: PlannerFactValue::Quantity {
                    value: 8,
                    unit: conduit_core::CharacteristicUnit::Items,
                },
            }],
            ..HardRealizationRequirements::default()
        },
    )])
}

fn policies() -> BTreeMap<GearId, RealizationPolicy> {
    BTreeMap::from([
        (GearId::from("text_lab/keyboard"), prefer_host(LOCAL)),
        (GearId::from("text_lab/presentation"), prefer_host(LOCAL)),
    ])
}

fn plan_fixture(
    form: &conduit_form::CheckedForm,
    hosts: &[conduit_core::HostAdvertisement],
    observations: &[ResourceObservation],
    lines: &[conduit_core::LineOffer],
) -> Result<conduit_core::Plan, conduit_planner::PlannerError> {
    let line_candidates = BTreeMap::from([
        (
            (
                GearId::from("text_lab/keyboard"),
                GearId::from("text_lab/keymap"),
            ),
            vec![lines[0].line_id.clone()],
        ),
        (
            (
                GearId::from("text_lab/uppercase"),
                GearId::from("text_lab/presentation"),
            ),
            vec![lines[1].line_id.clone()],
        ),
    ]);
    plan_selected_realizations_with_characteristics_and_options(
        form,
        hosts,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        &requirements(),
        &[],
        observations,
        &policies(),
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 4,
            connection_byte_capacity: 24,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: lines,
        },
    )
}

#[test]
fn human_facing_gears_stay_local_while_heavy_work_uses_an_ordinary_peer_plan() {
    let form = form();
    let hosts = hosts();
    let observations = observations(&hosts);
    let lines = lines(&hosts);
    let plan = plan_fixture(&form, &hosts, &observations, &lines)
        .expect("explicit locality and capacity policy produce one ordinary Plan");

    assert!(verify_plan(&plan));
    let placement = |gear: &str| {
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|planned| planned.gear_id.as_str() == gear)
            .expect("planned Gear")
            .host_id
            .as_str()
    };
    assert_eq!(placement("text_lab/keyboard"), LOCAL);
    assert_eq!(placement("text_lab/uppercase"), REMOTE);
    assert_eq!(placement("text_lab/presentation"), LOCAL);
    let remote_cords = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter(|connection| !connection.admitted_lines.is_empty())
        .map(|connection| connection.connection_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(remote_cords.len(), 2);
}

#[test]
fn insufficient_or_lost_line_refuses_replacement_without_mutating_the_old_plan() {
    let form = form();
    let hosts = hosts();
    let observations = observations(&hosts);
    let mut lines = lines(&hosts);
    let accepted = plan_fixture(&form, &hosts, &observations, &lines).expect("initial Plan");
    let accepted_id = accepted.plan_id.clone();
    let wake = lifecycle::active_wake(&accepted, LOCAL);

    lines[1].binding.limits.maximum_payload_bytes = 8;
    let insufficient = plan_fixture(&form, &hosts, &observations, &lines)
        .expect_err("an undersized return Line refuses before Play");
    assert!(matches!(
        insufficient,
        conduit_planner::PlannerError::LineOfferUnavailable(_)
    ));

    lines[1].binding.limits.maximum_payload_bytes = 256;
    lines[0].availability.availability = conduit_core::LineAvailability::Unavailable;
    let lost = plan_fixture(&form, &hosts, &observations, &lines)
        .expect_err("lost selected Line requires ordinary replacement planning");
    assert!(matches!(
        lost,
        conduit_planner::PlannerError::LineOfferUnavailable(_)
    ));
    assert_eq!(accepted.plan_id, accepted_id);
    assert!(verify_plan(&accepted));
    let wake = wake
        .became_unsatisfied(
            &accepted.plan_id,
            SignId::from("human-locality/play-line-unsatisfied"),
        )
        .expect("ordinary lifecycle records the active Play as unsatisfied");
    assert_eq!(wake.lifecycle, conduit_body::WakeLifecycle::Unsatisfied);
}

#[test]
fn local_human_preference_retains_its_exact_policy_source() {
    let form = form();
    let hosts = hosts();
    let observations = observations(&hosts);
    let presentation = form
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "text_lab/presentation")
        .unwrap();
    let semantic = lifecycle::policy_source(
        "checked-form/text-lab",
        1,
        PolicyScope::SemanticRequirements,
    );
    let workspace =
        lifecycle::policy_source("workspace/human-locality", 4, PolicyScope::UserWorkspace);
    let monitor =
        lifecycle::policy_source("monitor/current-resources", 7, PolicyScope::SiteDeployment);
    let selection = select_realization_with_scoped_policy(
        presentation,
        &hosts,
        &[],
        &HardRealizationRequirements::default(),
        semantic.clone(),
        &[PolicyLayer {
            source: workspace.clone(),
            hard_predicates: vec![],
            preferences: prefer_host(LOCAL).preferences,
        }],
        &lifecycle::reviewed_observations(&observations, &monitor),
        11,
    )
    .expect("attributable workspace policy selects local presentation");
    assert_eq!(selection.selection.choice.host_id.as_str(), LOCAL);
    assert_eq!(
        selection.basis.policy_sources,
        vec![semantic, workspace.clone()]
    );
    let selected = selection
        .selection
        .signs
        .iter()
        .find(|record| record.disposition == RealizationDecisionDisposition::Selected)
        .unwrap();
    assert_eq!(
        selected.decisive_preference_source.as_ref(),
        Some(&workspace)
    );
}

#[test]
fn hard_locality_wins_over_remote_power_and_remote_loss_preserves_local_truth() {
    let form = form();
    let hosts = hosts();
    let current_observations = observations(&hosts);
    let presentation = form
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "text_lab/presentation")
        .unwrap();
    let selection = select_realization_with_characteristics_and_signs(
        presentation,
        &hosts,
        &[],
        &HardRealizationRequirements {
            predicates: vec![require_host(LOCAL)],
            ..HardRealizationRequirements::default()
        },
        &current_observations,
        &prefer_host(REMOTE),
    )
    .expect("hard locality is not waived for a stronger remote preference");
    assert_eq!(selection.choice.host_id.as_str(), LOCAL);
    assert!(selection.signs.iter().any(|record| {
        record.host_id.as_str() == REMOTE
            && matches!(
                record.disposition,
                RealizationDecisionDisposition::Rejected(_)
            )
    }));

    let local_only = &hosts[..1];
    let local_observations = observations(local_only);
    let keyboard = form
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "text_lab/keyboard")
        .unwrap();
    let still_local = select_realization_with_characteristics_and_signs(
        keyboard,
        local_only,
        &[],
        &HardRealizationRequirements::default(),
        &local_observations,
        &prefer_host(LOCAL),
    )
    .expect("remote disappearance does not erase the local input offer");
    assert_eq!(still_local.choice.host_id.as_str(), LOCAL);

    let heavy = form
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "text_lab/uppercase")
        .unwrap();
    let refusal = select_realization_with_characteristics_and_signs(
        heavy,
        local_only,
        &[],
        requirements()
            .get(&GearId::from("text_lab/uppercase"))
            .unwrap(),
        &local_observations,
        &RealizationPolicy::default(),
    )
    .expect_err("the missing capable peer yields a specific capacity refusal");
    assert!(matches!(
        refusal,
        conduit_planner::PlannerError::HardRealizationRequirementUnsatisfied(_)
    ));
}

#[test]
fn remote_capacity_cannot_override_authority_or_data_locality_requirements() {
    let form = form();
    let mut hosts = hosts();
    let authority = conduit_core::AuthorityContractId::from("authority/text-may-leave-laptop");
    let remote_upper = hosts[1]
        .capabilities
        .iter_mut()
        .find(|offer| offer.kind_id.as_str() == "text/upper")
        .unwrap();
    let operation = remote_upper.host_operations[0].contract_id.clone();
    remote_upper
        .authority_requirements
        .push(conduit_core::AuthorityRequirement {
            contract_id: authority.clone(),
            host_operation_contract_id: operation,
            subject_kind: conduit_core::kind_id("value/text@1"),
        });
    let observations = observations(&hosts);
    let heavy = form
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "text_lab/uppercase")
        .unwrap();
    let refusal = select_realization_with_characteristics_and_signs(
        heavy,
        &hosts,
        &[],
        &HardRealizationRequirements {
            predicates: vec![
                PlannerPredicate::AtLeast {
                    fact: PlannerFactRef::ObservationUnreservedUnits(ResourceClassId::from(CPU)),
                    value: PlannerFactValue::Quantity {
                        value: 8,
                        unit: conduit_core::CharacteristicUnit::Items,
                    },
                },
                PlannerPredicate::Equal {
                    fact: PlannerFactRef::RequiresAuthority(authority),
                    value: PlannerFactValue::Boolean(false),
                },
            ],
            ..HardRealizationRequirements::default()
        },
        &observations,
        &prefer_host(REMOTE),
    )
    .expect_err("remote power cannot waive an explicit no-egress authority policy");
    assert!(matches!(
        refusal,
        conduit_planner::PlannerError::HardRealizationRequirementUnsatisfied(_)
    ));
}
