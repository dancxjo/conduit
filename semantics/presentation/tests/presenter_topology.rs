#![cfg(feature = "form-catalog")]

mod common;

use conduit_core::{
    ArtifactId, BaseImplementationId, CapabilityId, CapabilityLimits, ExecutionProfileId, GearId,
    ImplementationId, ImplementationOffer,
};
use conduit_form::{parse, ProfileCatalog};
use conduit_planner::{plan, PlacementChoice, PlacementChoices};
use conduit_presentation::{
    presenter_stage_kind_projection, presenter_stage_offer, renderer_kind_projection,
    PresenterTopologyAdmission, MAX_RENDERER_VALUE_BYTES,
};
use std::collections::BTreeMap;

const SOURCE: &str = "form spoken-front {\n normalize: presentation/presenter-stage\n speech: presentation/renderer\n normalize.presentation >> speech.presentation\n}\n";

#[test]
fn ordinary_plan_cords_seal_a_typed_two_stage_presenter_chain() {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(presenter_stage_kind_projection()).unwrap();
    catalog.insert(renderer_kind_projection()).unwrap();
    let form = parse(SOURCE, &catalog).unwrap();
    let mut host = common::host(
        "speech-host",
        "speech-boot",
        "speech",
        "speech@1",
        "speech-artifact@1",
        "presentation/base/test-speech@1",
        common::WAYLAND_RESOURCE,
    );
    host.capabilities[0].limits.max_queue_items = 4;
    host.capabilities.push(presenter_stage_offer(
        CapabilityId::from("normalize"),
        ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("presentation/normalize@1"),
            implementation_id: ImplementationId::from("normalize@1"),
            artifact_id: ArtifactId::from("normalize-artifact@1"),
        },
        CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
        },
    ));
    host.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("spoken-front/normalize"),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: CapabilityId::from("normalize"),
                },
            ),
            (
                GearId::from("spoken-front/speech"),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: CapabilityId::from("speech"),
                },
            ),
        ]),
    };
    let sealed = plan(
        &form,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    let topology = PresenterTopologyAdmission::from_plan(&sealed).unwrap();
    assert_eq!(topology.plan_id, sealed.plan_id);
    assert_eq!(topology.chains.len(), 1);
    assert_eq!(topology.chains[0].stages.len(), 2);
    assert_eq!(
        topology.chains[0].stages[0].implementation_id.as_str(),
        "normalize@1"
    );
    assert_eq!(
        topology.chains[0].stages[1].implementation_id.as_str(),
        "speech@1"
    );
    assert_eq!(topology.chains[0].stages[0].input_item_capacity, 4);
    assert!(topology.chains[0].stages[0].resources.is_empty());
    assert_eq!(topology.chains[0].stages[1].resources.len(), 1);
    assert!(!SOURCE.contains("speech-host"));
}

#[test]
fn two_renderer_placements_are_two_independently_admitted_chains() {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(renderer_kind_projection()).unwrap();
    let form = parse(
        "form front {\n graphical: presentation/renderer\n speech: presentation/renderer\n}\n",
        &catalog,
    )
    .unwrap();
    let graphical = common::host(
        "graphical-host",
        "graphical-boot",
        "graphical",
        "graphical@1",
        "graphical-artifact@1",
        "presentation/base/test-graphical@1",
        common::WAYLAND_RESOURCE,
    );
    let speech = common::host(
        "speech-host",
        "speech-boot",
        "speech",
        "speech@1",
        "speech-artifact@1",
        "presentation/base/test-speech@1",
        common::DOM_RESOURCE,
    );
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("front/graphical"),
                PlacementChoice {
                    host_id: graphical.host_id.clone(),
                    capability_id: CapabilityId::from("graphical"),
                },
            ),
            (
                GearId::from("front/speech"),
                PlacementChoice {
                    host_id: speech.host_id.clone(),
                    capability_id: CapabilityId::from("speech"),
                },
            ),
        ]),
    };
    let sealed = plan(&form, &[graphical, speech], &placements, &[]).unwrap();
    let topology = PresenterTopologyAdmission::from_plan(&sealed).unwrap();
    assert_eq!(topology.chains.len(), 2);
    assert!(topology.chains.iter().all(|chain| chain.stages.len() == 1));
}
