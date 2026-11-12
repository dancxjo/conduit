//! Actual sealed Body containment over mixed local and selected-Line fragments.
use super::*;
use conduit_body::{
    Body, BodyPlan, BodyPlayIdentity, BodyPlotPlan, BodyWorkset, PreparedBodyFragment, ResidentPlot,
};
fn limits() -> b::BodyFragmentProjectionLimits {
    b::BodyFragmentProjectionLimits {
        image: b::EmbeddedImageBounds::HOST_TOOLING,
        computations: b::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
        assigned: maxima(),
        maximum_fore_ports: 2,
        maximum_document_bytes: 16384,
    }
}
fn actual() -> c::HostAdvertisement {
    c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/pure".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn second_plan(bytes: u32) -> c::Plan {
    let source = "plot increment (\n >> reading: U8\n result: U8 >>\n) = (reading + 2)\n";
    let checked =
        p::check_syntax_document(&p::parse_syntax_document(source), &p::StartupCatalog::new())
            .unwrap();
    let plot =
        p::expand_canonical_plot_for_authoring(&checked, "increment", &p::ProfileCatalog::new())
            .unwrap();
    let gear = &plot.expanded.gears[0];
    let c::ConfigurationValue::Text(program) = &gear.configuration[0].value else {
        panic!("program")
    };
    let program = p::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let _initialized = p::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let offer = c::capability_offer_from_parts! {
        semantic_contract:gear.semantic_contract.clone(),startup_parameters:gear.startup_parameters.clone(),shorthand:gear.shorthand.clone(),
        capability_id:"fixture/increment".into(),kind_id:gear.kind_id.clone(),kind_contract_revision:gear.kind_contract_revision.clone(),
        implementation:c::ImplementationOffer {execution_profile_id:"fixture/pure".into(),implementation_id:"fixture/increment".into(),artifact_id:"fixture/increment".into()},
        inputs:gear.inputs.clone(),outputs:gear.outputs.clone(),host_calls:vec![],resource_requirements:vec![],authority_requirements:vec![],
        limits:c::CapabilityLimits {max_active_instances:1,max_queue_items:1,max_queue_bytes:1024},
    };
    let host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/pure".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let hosts = [host];
    let choices = conduit_planner::default_expanded_placements(&plot.expanded, &hosts).unwrap();
    let mut fore = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &plot.input_bindings),
        (c::PortDirection::Output, &plot.output_bindings),
    ] {
        for binding in bindings {
            fore.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
    }
    conduit_planner::plan_expanded_authoring_with_options(
        &plot,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: bytes,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &fore,
    )
    .unwrap()
}
#[test]
fn sealed_body_keeps_schema8_line_fore_and_schema7_local_sibling_in_complete_set() {
    let line_plan = selected_plan();
    let local_plan = second_plan(64);
    let partitions: Vec<_> = [&line_plan, &local_plan]
        .into_iter()
        .map(|p| BodyPlotPlan {
            plot: ResidentPlot::new(p.source_document_id.clone(), p.checked_plot_id.clone()),
            plan: p.clone(),
        })
        .collect();
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(partitions.iter().map(|p| p.plot.clone())).unwrap(),
        1,
        "fixture/born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "fixture/wake".into()).unwrap().1;
    let body_plan = BodyPlan::seal(&wake, partitions).unwrap();
    let play = BodyPlayIdentity::bind(&body_plan, 1);
    let a =
        PreparedBodyFragment::prepare(&wake, &body_plan, &play, &line_plan.fragments[0]).unwrap();
    let sibling =
        PreparedBodyFragment::prepare(&wake, &body_plan, &play, &local_plan.fragments[0]).unwrap();
    let document =
        b::generate_prepared_body_fragment_with_line_fore(a, &[], limits(), &[session()]).unwrap();
    let (metadata, projection) =
        conduit_body::decode_body_fragment_document(document.bytes()).unwrap();
    assert_eq!(metadata.body, body.body_id.as_str());
    assert_eq!(metadata.body_plan, body_plan.plan_id.as_str());
    assert_eq!(metadata.body_play, play.active_play_id.as_str());
    assert_eq!(
        metadata.fragment,
        line_plan.fragments[0].fragment_id.as_str()
    );
    assert_eq!(metadata.local_fragment_count, 2);
    assert_eq!(
        u16::from_le_bytes([projection[8], projection[9]]),
        c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA
    );
    assert_eq!(
        conduit_body::encode_body_fragment_document(&metadata, projection, 16384).unwrap(),
        document.bytes()
    );
    let generated = project(&line_plan, &[session()]).unwrap();
    let inventory: Vec<_> = generated
        .fore()
        .iter()
        .flat_map(|p| p.payload().iter().copied())
        .collect();
    assert_eq!(metadata.fore_ports, inventory);
    assert!(metadata.resource_bindings.is_empty());
    let computations = b::assigned_computation_requirements(generated.kernel()).unwrap();
    c::decode_assigned_plan_with_line_fore(
        projection,
        maxima(),
        c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text("fixture/host"),
            boot: c::AssignedIdentity::from_text("fixture/boot"),
            host_calls: &[],
            resources: &[],
            remote_bindings: generated.remote_bindings(),
        },
        c::AssignedComputationProfile {
            maximum_text_bytes: 65535,
            maximum_total_text_bytes: u32::MAX,
            requirements: &computations,
        },
        generated.profile(),
    )
    .unwrap();
    let resolve = |scope: PreparedBodyFragment<'_>| {
        Ok((
            vec![],
            limits(),
            if scope.fragment().fragment_id == line_plan.fragments[0].fragment_id {
                vec![session()]
            } else {
                vec![]
            },
        ))
    };
    let mut docs = Vec::new();
    let set = b::generate_prepared_body_workset_with_line_fore(a, &actual(), 32768, |scope| {
        let selected = if scope.fragment().fragment_id == line_plan.fragments[0].fragment_id {
            vec![session()]
        } else {
            assert_eq!(
                scope.fragment().fragment_id,
                local_plan.fragments[0].fragment_id
            );
            vec![]
        };
        docs.push(
            b::generate_prepared_body_fragment_with_line_fore(scope, &[], limits(), &selected)
                .unwrap(),
        );
        Ok((vec![], limits(), selected))
    })
    .unwrap();
    assert_eq!(docs.len(), 2);
    let schemas: Vec<_> = docs
        .iter()
        .map(|d| {
            let (_, p) = conduit_body::decode_body_fragment_document(d.bytes()).unwrap();
            u16::from_le_bytes([p[8], p[9]])
        })
        .collect();
    assert!(schemas.contains(&7) && schemas.contains(&8));
    let refs: Vec<_> = docs.iter().map(|d| d.bytes()).collect();
    assert_eq!(
        set.bytes(),
        conduit_body::encode_body_workset_document(&refs, 32768).unwrap()
    );
    assert_eq!(
        set.bytes(),
        b::generate_prepared_body_workset_with_line_fore(sibling, &actual(), 32768, resolve)
            .unwrap()
            .bytes()
    );
    assert!(conduit_body::encode_body_workset_document(&[document.bytes()], 32768).is_err());
    assert!(b::generate_prepared_body_fragment(a, &[], limits()).is_err());
    assert!(b::generate_prepared_body_fragment_with_line_fore(a, &[], limits(), &[]).is_err());
    for kind in 0..4 {
        let mut wrong = session();
        match kind {
            0 => wrong.front_port_id = "foreign".into(),
            1 => wrong.direction = c::PortDirection::Input,
            2 => wrong.track = c::ConnectionTrack::NormalClose,
            _ => {}
        }
        let sessions = if kind == 3 {
            vec![session(), wrong]
        } else {
            vec![wrong]
        };
        assert!(
            b::generate_prepared_body_fragment_with_line_fore(a, &[], limits(), &sessions).is_err()
        );
    }
    assert!(b::generate_prepared_body_fragment_with_line_fore(
        sibling,
        &[],
        limits(),
        &[session()]
    )
    .is_err());
    let mut short = limits();
    short.maximum_document_bytes = document.bytes().len() as u32 - 1;
    assert!(
        b::generate_prepared_body_fragment_with_line_fore(a, &[], short, &[session()]).is_err()
    );
    for field in 0..3 {
        let mut stale = actual();
        match field {
            0 => stale.host_id = "foreign".into(),
            1 => stale.boot_id = "foreign".into(),
            _ => stale.offer_generation.0 += 1,
        }
        assert!(matches!(
            b::generate_prepared_body_workset_with_line_fore(a, &stale, 32768, |_| panic!(
                "stale identity reached resolver"
            )),
            Err(b::BodyFragmentGenerationRefusal::Scope)
        ));
    }
    assert!(
        b::generate_prepared_body_workset_with_line_fore(a, &actual(), 0, |_| panic!(
            "zero bound reached resolver"
        ))
        .is_err()
    );
    assert!(b::generate_prepared_body_workset_with_line_fore(
        a,
        &actual(),
        set.bytes().len() as u32 - 1,
        resolve
    )
    .is_err());
}
