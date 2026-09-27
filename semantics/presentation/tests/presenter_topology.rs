#![cfg(feature = "form-catalog")]

mod common;

use conduit_core::{
    bind_active_play, kind_id, port_id, ArtifactId, Back, BackOfferBuilder, BaseImplementationId,
    CapabilityId, CapabilityLimits, ExecutionProfileId, GearId, ImplementationId,
    ImplementationOffer, Kind, KindIdentity, PortDescriptor, PortDirection, PortTemporal, SignId,
};
use conduit_form::{parse, KindProjection, ProfileCatalog};
use conduit_planner::{plan, PlacementChoice, PlacementChoices};
use conduit_presentation::{
    presenter_stage_kind_projection, presenter_stage_offer, renderer_kind_projection,
    ManifestationLifecycle, MaskBoundaryPort, MaskBoundaryRole, MaskCordSpecification, MaskShow,
    MaskSpecification, MaskStageId, MaskStagePlacement, MaskStageSpecification,
    PresenterTopologyAdmission, MAX_RENDERER_VALUE_BYTES,
};
use std::collections::BTreeMap;

const SOURCE: &str = "form spoken-front {\n normalize: presentation/presenter-stage\n speech: presentation/renderer\n normalize.presentation >> speech.presentation\n}\n";

fn stage_kind(kind: &str, input: &str, output: &str) -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(format!("conduit.test/{kind}@1")),
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: kind_id(input),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("output"),
            value_kind: kind_id(output),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        configuration: vec![],
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 8,
            max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
        },
    }
}

fn stage_offer(kind: Kind, name: &str) -> conduit_core::CapabilityOffer {
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(name),
            execution_profile_id: ExecutionProfileId::from(format!("test/{name}@1")),
            implementation_id: ImplementationId::from(format!("{name}@1")),
            artifact_id: ArtifactId::from(format!("{name}-artifact@1")),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

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

#[test]
fn one_mask_chain_preserves_heterogeneous_language_text_and_pcm_stages() {
    let language = stage_kind(
        "presentation/generative-language",
        "presentation/presentation@1",
        "text/text@1",
    );
    let speech = stage_kind("speech/synthesize", "text/text@1", "audio/pcm@1");
    let playback = stage_kind("audio/play", "audio/pcm@1", "presentation/manifestation@1");
    let stage_id = |value| MaskStageId::new(value).unwrap();
    let specification = MaskSpecification::new(
        "generative-spoken",
        1,
        vec![
            MaskStageSpecification {
                stage_id: stage_id("language"),
                kind_id: language.kind_id.clone(),
                kind_contract_revision: language.kind_contract_revision.clone(),
                inputs: language.inputs.clone(),
                outputs: language.outputs.clone(),
            },
            MaskStageSpecification {
                stage_id: stage_id("voice"),
                kind_id: speech.kind_id.clone(),
                kind_contract_revision: speech.kind_contract_revision.clone(),
                inputs: speech.inputs.clone(),
                outputs: speech.outputs.clone(),
            },
            MaskStageSpecification {
                stage_id: stage_id("output"),
                kind_id: playback.kind_id.clone(),
                kind_contract_revision: playback.kind_contract_revision.clone(),
                inputs: playback.inputs.clone(),
                outputs: playback.outputs.clone(),
            },
        ],
        vec![
            MaskCordSpecification {
                source_stage_id: stage_id("language"),
                source_port_id: port_id("output"),
                sink_stage_id: stage_id("voice"),
                sink_port_id: port_id("input"),
                value_kind: kind_id("text/text@1"),
            },
            MaskCordSpecification {
                source_stage_id: stage_id("voice"),
                source_port_id: port_id("output"),
                sink_stage_id: stage_id("output"),
                sink_port_id: port_id("input"),
                value_kind: kind_id("audio/pcm@1"),
            },
        ],
        vec![
            MaskBoundaryPort {
                role: MaskBoundaryRole::PresentationInput,
                stage_id: stage_id("language"),
                port_id: port_id("input"),
            },
            MaskBoundaryPort {
                role: MaskBoundaryRole::ShowOutput,
                stage_id: stage_id("output"),
                port_id: port_id("output"),
            },
        ],
    )
    .unwrap();
    let mut catalog = ProfileCatalog::new();
    for kind in [&language, &speech, &playback] {
        catalog.insert(KindProjection::from(kind)).unwrap();
    }
    let source = "form spoken-mask {\n language: presentation/generative-language\n voice: speech/synthesize\n output: audio/play\n language.output >> voice.input\n voice.output >> output.input\n}\n";
    let form = parse(source, &catalog).unwrap();
    let mut host = common::host(
        "speech-host",
        "speech-boot",
        "unused-renderer",
        "unused-renderer@1",
        "unused-renderer-artifact@1",
        "presentation/base/test-speech@1",
        common::WAYLAND_RESOURCE,
    );
    host.capabilities.extend([
        stage_offer(language, "language"),
        stage_offer(speech, "voice"),
        stage_offer(playback, "output"),
    ]);
    host.capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("spoken-mask/language"),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: CapabilityId::from("language"),
                },
            ),
            (
                GearId::from("spoken-mask/voice"),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: CapabilityId::from("voice"),
                },
            ),
            (
                GearId::from("spoken-mask/output"),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: CapabilityId::from("output"),
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
    assert_eq!(topology.chains.len(), 1);
    assert_eq!(topology.chains[0].stages.len(), 3);
    assert_eq!(
        topology.chains[0]
            .stages
            .iter()
            .map(|stage| (stage.input_kind.as_str(), stage.output_kind.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("presentation/presentation@1", "text/text@1"),
            ("text/text@1", "audio/pcm@1"),
            ("audio/pcm@1", "presentation/manifestation@1"),
        ]
    );

    let placement_for = |gear: &str| {
        sealed
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| placement.gear_id.as_str() == gear)
            .unwrap()
            .placement_id
            .clone()
    };
    let admitted = specification
        .admit_plan(
            &sealed,
            vec![
                MaskStagePlacement {
                    stage_id: stage_id("language"),
                    placement_id: placement_for("spoken-mask/language"),
                },
                MaskStagePlacement {
                    stage_id: stage_id("voice"),
                    placement_id: placement_for("spoken-mask/voice"),
                },
                MaskStagePlacement {
                    stage_id: stage_id("output"),
                    placement_id: placement_for("spoken-mask/output"),
                },
            ],
        )
        .unwrap();
    assert_eq!(admitted.specification_id, specification.specification_id);
    assert_eq!(admitted.plan_id, sealed.plan_id);
    assert_ne!(
        admitted.specification_id.as_str(),
        admitted.plan_id.as_str()
    );
    assert_eq!(admitted.stages.len(), 3);
    assert_eq!(admitted.cords.len(), 2);
    assert_eq!(admitted.stages[2].implementation_id.as_str(), "output@1");

    let presentation = common::presentation(&form, &sealed);
    let terminal = admitted
        .stages
        .iter()
        .find(|stage| stage.stage_id.as_str() == "output")
        .unwrap();
    let active_play = bind_active_play(&sealed.plan_id, &terminal.host_id, &terminal.boot_id, 1);
    let show = MaskShow::prepared(
        &specification,
        &admitted,
        &presentation,
        &sealed,
        active_play,
        "patchbay/form".into(),
        "speaker/default".into(),
        SignId::from("mask/show/prepared"),
    )
    .unwrap();
    let available = show
        .transition(
            ManifestationLifecycle::Available,
            SignId::from("mask/show/available"),
        )
        .unwrap();
    available
        .validate(&specification, &presentation, &sealed)
        .unwrap();
    assert_eq!(available.show_id, show.show_id);
    assert_eq!(available.presentation_id, presentation.identity);
}
