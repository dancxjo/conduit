use conduit_core::PortTemporal;
use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, ImplementationOffer, KindContractRevision, OfferGeneration, PROTOCOL_VERSION,
};
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    install_navigation_catalogs, install_robotics_structured_catalogs, local_control,
    navigation_control_type, navigation_kind_contracts, route_grid4, time_parameterize,
    BoundedMotionIntent, ControlDecision, GoalTarget, NavigationGoal, NavigationPose,
    NavigationRefusal, NavigationTime, RouteDecision, Traversability4x4, TraversabilityCell,
    Validity, Waypoint, NAVIGATION_REVISION,
};

const SOURCE: &str = include_str!("../../../forms/bounded-navigation/main.conduit");

#[test]
fn canonical_bounded_navigation_is_one_checked_form() {
    let (startup, profile) = catalogs();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let mut profile = profile;
    let selector_definitions = checked
        .forms
        .iter()
        .flat_map(|form| &form.cords)
        .flat_map(|cord| &cord.stages)
        .filter_map(|stage| match stage {
            CheckedCordStage::StructuredSelector { selector, .. } => Some(selector),
            _ => None,
        })
        .map(|selector| structured_selector_definition(selector, PortTemporal::Value))
        .collect::<Vec<_>>();
    for definition in &selector_definitions {
        profile.insert(definition.clone()).unwrap();
    }
    let authored =
        expand_canonical_form_for_authoring(&checked, "bounded-navigation", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 5);
    for kind in [
        conduit_semantic_catalog::NAVIGATION_ROUTE_GRID4_KIND,
        conduit_semantic_catalog::NAVIGATION_TIME_PARAMETERIZE_KIND,
        conduit_semantic_catalog::NAVIGATION_LOCAL_CONTROL_KIND,
    ] {
        assert!(authored
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == kind));
    }
    assert_eq!(authored.input_bindings.len(), 7);
    assert_eq!(authored.output_bindings.len(), 3);
    assert!(!SOURCE.contains("PlanId"));
    let lowercase = SOURCE.to_ascii_lowercase();
    for forbidden in ["create-oi", "wheel", "uart", "host/", "socket"] {
        assert!(!lowercase.contains(forbidden), "Form leaked {forbidden}");
    }

    let host = host(selector_definitions);
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &authored.expanded,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    assert!(plan.fragments[0]
        .placements
        .iter()
        .all(|placement| placement.authority.is_empty()));
}

#[test]
fn deterministic_route_trajectory_and_controller_are_bounded_and_feedback_aware() {
    let start_pose = pose(50, 50, 0);
    let goal = goal(250, 250, 0);
    let mut grid = grid(TraversabilityCell::Free);
    grid.cells[1] = TraversabilityCell::Blocked;
    let time = time(500);

    let RouteDecision::Route(route) = route_grid4(&start_pose, &goal, &grid, &time).unwrap() else {
        panic!("bounded alternative route must exist")
    };
    assert_eq!(route.planner_identity, "navigation/deterministic-grid4@1");
    assert_eq!(
        route.planning_input_identity,
        "goal=goal/B;pose=pose/fixture#7;grid=grid/fixture#11"
    );
    assert_eq!(route.waypoints.len(), 3);
    assert_eq!(
        route.waypoints[1],
        Waypoint {
            x_mm: 50,
            y_mm: 250
        }
    );
    let trajectory = time_parameterize(&route, &time, 100, 50, 30_000_000).unwrap();
    assert_eq!(trajectory.segments.len(), 2);

    assert_eq!(
        local_control(&start_pose, &trajectory, &time).unwrap(),
        ControlDecision::Motion(BoundedMotionIntent {
            goal_identity: "goal/B".into(),
            linear_mm: 0,
            angular_microdegrees: 30_000_000,
            interval_ms: 100,
            ttl_ms: 100,
        })
    );

    let at_corner = pose(50, 250, 0);
    assert_eq!(
        local_control(&at_corner, &trajectory, &time).unwrap(),
        ControlDecision::Motion(BoundedMotionIntent {
            goal_identity: "goal/B".into(),
            linear_mm: 50,
            angular_microdegrees: 0,
            interval_ms: 100,
            ttl_ms: 100,
        })
    );

    let arrived = pose(248, 250, 1_000_000);
    assert_eq!(
        local_control(&arrived, &trajectory, &time).unwrap(),
        ControlDecision::Arrived {
            goal_identity: "goal/B".into()
        }
    );
}

#[test]
fn an_already_reached_position_retains_terminal_heading_work() {
    let start = pose(250, 250, 0);
    let now = time(500);
    let RouteDecision::Route(route) = route_grid4(
        &start,
        &goal(250, 250, 90_000_000),
        &grid(TraversabilityCell::Free),
        &now,
    )
    .unwrap() else {
        panic!("heading-only goal must retain a route")
    };
    assert_eq!(route.waypoints.len(), 2);
    let trajectory = time_parameterize(&route, &now, 100, 50, 30_000_000).unwrap();
    assert!(matches!(
        local_control(&start, &trajectory, &now).unwrap(),
        ControlDecision::Motion(BoundedMotionIntent {
            linear_mm: 0,
            angular_microdegrees: 30_000_000,
            ..
        })
    ));
}

#[test]
fn stale_invalid_unknown_and_no_path_remain_distinct() {
    let current = time(500);
    let goal = goal(250, 250, 0);
    let fresh_pose = pose(50, 50, 0);

    let mut stale_pose = fresh_pose.clone();
    stale_pose.validity.valid_until_ms = 499;
    assert!(matches!(
        route_grid4(
            &stale_pose,
            &goal,
            &grid(TraversabilityCell::Free),
            &current
        )
        .unwrap(),
        RouteDecision::PoseStale { .. }
    ));

    let mut expired_goal = goal.clone();
    expired_goal.valid_until_ms = 499;
    assert!(matches!(
        route_grid4(
            &fresh_pose,
            &expired_goal,
            &grid(TraversabilityCell::Free),
            &current
        )
        .unwrap(),
        RouteDecision::GoalInvalid { .. }
    ));

    assert!(matches!(
        route_grid4(
            &fresh_pose,
            &goal,
            &grid(TraversabilityCell::Unknown),
            &current
        )
        .unwrap(),
        RouteDecision::ObstacleDataUnavailable { .. }
    ));
    assert!(matches!(
        route_grid4(
            &fresh_pose,
            &goal,
            &grid(TraversabilityCell::Blocked),
            &current
        )
        .unwrap(),
        RouteDecision::NoPath { .. }
    ));
}

#[test]
fn invalid_grid_bounds_refuse_before_coordinate_arithmetic() {
    let mut invalid = grid(TraversabilityCell::Free);
    invalid.origin_x_mm = i32::MAX;
    invalid.cell_width_mm = u32::MAX;
    assert_eq!(
        route_grid4(&pose(50, 50, 0), &goal(250, 250, 0), &invalid, &time(500)),
        Err(NavigationRefusal::InvalidGrid)
    );
}

#[test]
fn structured_route_trajectory_and_control_codecs_preserve_exact_profiles() {
    let time = time(500);
    let start = pose(50, 50, 0);
    let RouteDecision::Route(route) = route_grid4(
        &start,
        &goal(250, 250, 0),
        &grid(TraversabilityCell::Free),
        &time,
    )
    .unwrap() else {
        panic!("fixture route")
    };
    let encoded_decision =
        conduit_semantic_catalog::encode_route_decision(&RouteDecision::Route(route.clone()))
            .unwrap();
    let decision = StructuredInfoValue::from_canonical_bytes(&encoded_decision).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = decision.shape() else {
        panic!("route decision is a variant")
    };
    assert_eq!(tag, "route");
    let decoded_route =
        conduit_semantic_catalog::decode_navigation_route(&payload.canonical_bytes().unwrap())
            .unwrap();
    assert_eq!(decoded_route, route);

    let trajectory = time_parameterize(&route, &time, 100, 50, 30_000_000).unwrap();
    let encoded_trajectory = conduit_semantic_catalog::encode_trajectory(&trajectory).unwrap();
    assert_eq!(
        conduit_semantic_catalog::decode_navigation_trajectory(&encoded_trajectory).unwrap(),
        trajectory
    );
    let control = local_control(&start, &trajectory, &time).unwrap();
    let encoded_control = conduit_semantic_catalog::encode_control(&control).unwrap();
    let control_value = StructuredInfoValue::from_canonical_bytes(&encoded_control).unwrap();
    assert_eq!(control_value.value_type(), &navigation_control_type());
    assert!(matches!(
        control_value.shape(),
        StructuredInfoValueShape::Variant { tag: "motion", .. }
    ));
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_presentation::install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    install_robotics_structured_catalogs(&mut startup, &mut profile).unwrap();
    install_navigation_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn host(selector_definitions: Vec<conduit_form::KindDefinition>) -> HostAdvertisement {
    let mut capabilities = navigation_kind_contracts()
        .into_iter()
        .map(|(kind, inputs, outputs)| {
            offer(
                kind,
                inputs,
                outputs,
                KindContractRevision::from(NAVIGATION_REVISION),
                vec![],
            )
        })
        .collect::<Vec<_>>();
    capabilities.extend(selector_definitions.into_iter().map(|definition| {
        offer(
            definition.kind_id,
            definition.inputs,
            definition.outputs,
            definition.kind_contract_revision,
            definition
                .configuration
                .into_iter()
                .map(|field| FaceStartupParameter {
                    name: field.key,
                    value_type: "Text".into(),
                    has_default: false,
                })
                .collect(),
        )
    }));
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/navigation-proof"),
        boot_id: BootId::from("boot/navigation-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/navigation-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}

fn offer(
    kind: conduit_core::KindId,
    inputs: Vec<conduit_core::PortDescriptor>,
    outputs: Vec<conduit_core::PortDescriptor>,
    revision: KindContractRevision,
    startup_parameters: Vec<FaceStartupParameter>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters,
        shorthand: None,
        capability_id: CapabilityId::from(format!("proof/{}", kind.as_str())),
        kind_id: kind.clone(),
        kind_contract_revision: revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("proof/navigation@1"),
            implementation_id: ImplementationId::from(format!("proof/{}@1", kind.as_str())),
            artifact_id: ArtifactId::from("proof/navigation@1"),
        },
        inputs,
        outputs,
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}

fn time(now_ms: u64) -> NavigationTime {
    NavigationTime {
        clock_identity: "clock/fixture".into(),
        now_ms,
    }
}

fn pose(x_mm: i32, y_mm: i32, heading_microdegrees: i32) -> NavigationPose {
    NavigationPose {
        source_identity: "pose/fixture".into(),
        sample_sequence: 7,
        clock_identity: "clock/fixture".into(),
        frame: "map/local".into(),
        x_mm,
        y_mm,
        heading_microdegrees,
        validity: Validity {
            observed_at_ms: 400,
            valid_until_ms: 600,
        },
    }
}

fn goal(x_mm: i32, y_mm: i32, heading_microdegrees: i32) -> NavigationGoal {
    NavigationGoal {
        identity: "goal/B".into(),
        clock_identity: "clock/fixture".into(),
        valid_until_ms: 1_000,
        target: GoalTarget::Reach {
            frame: "map/local".into(),
            x_mm,
            y_mm,
            heading_microdegrees,
            position_tolerance_mm: 5,
            heading_tolerance_microdegrees: 2_000_000,
        },
    }
}

fn grid(cell: TraversabilityCell) -> Traversability4x4 {
    Traversability4x4 {
        source_identity: "grid/fixture".into(),
        sample_sequence: 11,
        clock_identity: "clock/fixture".into(),
        frame: "map/local".into(),
        origin_x_mm: 0,
        origin_y_mm: 0,
        cell_width_mm: 100,
        cell_height_mm: 100,
        validity: Validity {
            observed_at_ms: 400,
            valid_until_ms: 600,
        },
        cells: [cell; 16],
    }
}
