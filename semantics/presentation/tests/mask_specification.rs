use conduit_core::{
    kind_id, port_id, KindId, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_presentation::{
    MaskBoundaryPort, MaskBoundaryRole, MaskCordSpecification, MaskSpecification,
    MaskSpecificationError, MaskStageId, MaskStageSpecification,
};

fn port(name: &str, kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn stage(
    id: &str,
    kind: &str,
    input: (&str, &str),
    output: (&str, &str),
) -> MaskStageSpecification {
    MaskStageSpecification {
        stage_id: MaskStageId::new(id).unwrap(),
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(format!("conduit.test/{kind}@1")),
        inputs: vec![port(input.0, input.1, PortDirection::Input)],
        outputs: vec![port(output.0, output.1, PortDirection::Output)],
    }
}

fn cord(source: &str, sink: &str, kind: &str) -> MaskCordSpecification {
    MaskCordSpecification {
        source_stage_id: MaskStageId::new(source).unwrap(),
        source_port_id: port_id("output"),
        sink_stage_id: MaskStageId::new(sink).unwrap(),
        sink_port_id: port_id("input"),
        value_kind: KindId::from(kind),
    }
}

fn boundaries(first: &str, last: &str) -> Vec<MaskBoundaryPort> {
    vec![
        MaskBoundaryPort {
            role: MaskBoundaryRole::PresentationInput,
            stage_id: MaskStageId::new(first).unwrap(),
            port_id: port_id("input"),
        },
        MaskBoundaryPort {
            role: MaskBoundaryRole::ShowOutput,
            stage_id: MaskStageId::new(last).unwrap(),
            port_id: port_id("output"),
        },
    ]
}

fn three_stage_mask(
    name: &str,
    kinds: [(&str, &str); 3],
) -> Result<MaskSpecification, MaskSpecificationError> {
    MaskSpecification::new(
        name,
        1,
        vec![
            stage(
                "first",
                kinds[0].0,
                ("input", "presentation/presentation@1"),
                ("output", kinds[0].1),
            ),
            stage(
                "second",
                kinds[1].0,
                ("input", kinds[0].1),
                ("output", kinds[1].1),
            ),
            stage(
                "third",
                kinds[2].0,
                ("input", kinds[1].1),
                ("output", "presentation/manifestation@1"),
            ),
        ],
        vec![
            cord("first", "second", kinds[0].1),
            cord("second", "third", kinds[1].1),
        ],
        boundaries("first", "third"),
    )
}

#[test]
fn graphical_deterministic_spoken_and_generative_spoken_fit_one_mask_contract() {
    let graphical = three_stage_mask(
        "native-graphical",
        [
            ("presentation/layout", "graphics/scene@1"),
            ("presentation/composite", "graphics/frame@1"),
            ("display/scanout", "presentation/manifestation@1"),
        ],
    )
    .unwrap();
    let deterministic = three_stage_mask(
        "deterministic-spoken",
        [
            ("presentation/aural", "text/text@1"),
            ("speech/synthesize", "audio/pcm@1"),
            ("audio/play", "presentation/manifestation@1"),
        ],
    )
    .unwrap();
    let generative = three_stage_mask(
        "generative-spoken",
        [
            ("presentation/generative", "text/text@1"),
            ("speech/synthesize", "audio/pcm@1"),
            ("audio/play", "presentation/manifestation@1"),
        ],
    )
    .unwrap();

    assert_ne!(graphical.specification_id, deterministic.specification_id);
    assert_ne!(deterministic.specification_id, generative.specification_id);
    for specification in [graphical, deterministic, generative] {
        assert_eq!(specification.stages.len(), 3);
        assert_eq!(specification.cords.len(), 2);
        assert_eq!(specification.boundaries.len(), 2);
    }
}

#[test]
fn interaction_is_a_mask_boundary_and_not_a_face_fact_or_host_selection() {
    let mut stages = vec![stage(
        "renderer",
        "presentation/render",
        ("input", "presentation/presentation@1"),
        ("output", "presentation/manifestation@1"),
    )];
    stages.push(stage(
        "pointer",
        "presentation/pointer-interaction",
        ("input", "browser/pointer-event@1"),
        ("output", "presentation/interaction@1"),
    ));
    let mut boundary = boundaries("renderer", "renderer");
    boundary.extend([
        MaskBoundaryPort {
            role: MaskBoundaryRole::LocalInteractionInput,
            stage_id: MaskStageId::new("pointer").unwrap(),
            port_id: port_id("input"),
        },
        MaskBoundaryPort {
            role: MaskBoundaryRole::FaceInteractionOutput,
            stage_id: MaskStageId::new("pointer").unwrap(),
            port_id: port_id("output"),
        },
    ]);

    let specification = MaskSpecification::new("browser", 3, stages, vec![], boundary).unwrap();
    assert_eq!(specification.revision, 3);
    assert!(specification
        .boundaries
        .iter()
        .any(|port| port.role == MaskBoundaryRole::FaceInteractionOutput));
}

#[test]
fn missing_show_and_half_an_interaction_boundary_refuse() {
    let renderer = stage(
        "renderer",
        "presentation/render",
        ("input", "presentation/presentation@1"),
        ("output", "presentation/manifestation@1"),
    );
    assert_eq!(
        MaskSpecification::new(
            "missing-show",
            1,
            vec![renderer.clone()],
            vec![],
            vec![MaskBoundaryPort {
                role: MaskBoundaryRole::PresentationInput,
                stage_id: MaskStageId::new("renderer").unwrap(),
                port_id: port_id("input"),
            }],
        ),
        Err(MaskSpecificationError::MissingShowOutput)
    );
    let mut boundary = boundaries("renderer", "renderer");
    boundary.push(MaskBoundaryPort {
        role: MaskBoundaryRole::LocalInteractionInput,
        stage_id: MaskStageId::new("renderer").unwrap(),
        port_id: port_id("input"),
    });
    assert_eq!(
        MaskSpecification::new("half-interaction", 1, vec![renderer], vec![], boundary),
        Err(MaskSpecificationError::IncompleteInteractionBoundary)
    );
}
