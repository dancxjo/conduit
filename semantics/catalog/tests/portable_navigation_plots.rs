use conduit_core::PortTemporal;
use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FrontStartupParameter, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, ImplementationOffer, KindIdentity, OfferGeneration, PROTOCOL_VERSION,
};
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::{
    install_navigation_catalogs, install_robotics_structured_catalogs, local_control,
    navigation_control_type, navigation_kind_contracts, route_grid4, time_parameterize,
    ControlDecision, GoalTarget, NavigationGoal, NavigationPose, NavigationTime, RouteDecision,
    Traversability4x4, TraversabilityCell, Validity, Waypoint, NAVIGATION_REVISION,
};

const SOURCE: &str = include_str!("../../../plots/bounded-navigation/main.conduit");

#[test]
fn canonical_bounded_navigation_is_one_checked_plot() {
    let (startup, profile) = catalogs();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let mut profile = profile;
    let selector_definitions = checked
        .plots
        .iter()
        .flat_map(|plot| &plot.cords)
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
        expand_canonical_plot_for_authoring(&checked, "bounded-navigation", &profile).unwrap();
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
        assert!(!lowercase.contains(forbidden), "Plot leaked {forbidden}");
    }

    let mut host = host(selector_definitions);
    for capability in &mut host.capabilities {
        if let Some(gear) = authored
            .expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id == capability.kind_id)
        {
            capability.semantic_contract = gear.semantic_contract.clone();
        }
    }
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
    let mut grid_cells = [TraversabilityCell::Free; 16];
    grid_cells[1] = TraversabilityCell::Blocked;
    let grid = grid_with_cells(grid_cells);
    let time = time(500);

    let RouteDecision::Route(route) = route_grid4(&start_pose, &goal, &grid, &time).unwrap() else {
        panic!("bounded alternative route must exist")
    };
    assert_eq!(
        route.planner_identity().get(),
        "navigation/deterministic-grid4@1"
    );
    assert_eq!(
        route.planning_input_identity().get(),
        "goal=goal/B;pose=pose/fixture#7;grid=grid/fixture#11"
    );
    assert_eq!(route.waypoints().len(), 3);
    assert_eq!(
        route.waypoints().iter().nth(1).unwrap(),
        &Waypoint::new(50, 250).unwrap()
    );
    let trajectory = time_parameterize(&route, &time, 100, 50, 30_000_000).unwrap();
    assert_eq!(trajectory.segments.len(), 2);

    let ControlDecision::Motion(first) = local_control(&start_pose, &trajectory, &time).unwrap()
    else {
        panic!("motion")
    };
    assert_eq!(
        (
            *first.linear_mm(),
            *first.angular_microdegrees(),
            *first.interval_ms(),
            *first.ttl_ms()
        ),
        (0, 30_000_000, 100, 100)
    );

    let at_corner = pose(50, 250, 0);
    let ControlDecision::Motion(corner) = local_control(&at_corner, &trajectory, &time).unwrap()
    else {
        panic!("motion")
    };
    assert_eq!(
        (*corner.linear_mm(), *corner.angular_microdegrees()),
        (50, 0)
    );

    let arrived = pose(248, 250, 1_000_000);
    assert!(matches!(
        local_control(&arrived, &trajectory, &time).unwrap(),
        ControlDecision::Arrived(_)
    ));
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
    assert_eq!(route.waypoints().len(), 2);
    let trajectory = time_parameterize(&route, &now, 100, 50, 30_000_000).unwrap();
    assert!(matches!(
        local_control(&start, &trajectory, &now).unwrap(),
        ControlDecision::Motion(_)
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
        RouteDecision::PoseStale(_)
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
        RouteDecision::GoalInvalid(_)
    ));

    assert!(matches!(
        route_grid4(
            &fresh_pose,
            &goal,
            &grid(TraversabilityCell::Unknown),
            &current
        )
        .unwrap(),
        RouteDecision::ObstacleDataUnavailable(_)
    ));
    assert!(matches!(
        route_grid4(
            &fresh_pose,
            &goal,
            &grid(TraversabilityCell::Blocked),
            &current
        )
        .unwrap(),
        RouteDecision::NoPath(_)
    ));
}

#[test]
fn invalid_grid_bounds_refuse_before_coordinate_arithmetic() {
    assert!(Traversability4x4::new(
        100,
        u32::MAX,
        cells([TraversabilityCell::Free; 16]),
        id("clock/fixture"),
        id("map/local"),
        i32::MAX,
        0,
        11,
        id("grid/fixture"),
        validity(),
    )
    .is_err());
}

#[test]
fn maximum_source_identities_remain_closed_through_trajectory_planning() {
    let identity = "x".repeat(64);
    let maximum = id(&identity);
    let pose = NavigationPose::new(
        maximum.clone(),
        maximum.clone(),
        0,
        u64::MAX,
        maximum.clone(),
        validity(),
        50,
        50,
    )
    .unwrap();
    let target = GoalTarget::reach(maximum.clone(), 0, 2_000_000, 5, 250, 250).unwrap();
    let goal = NavigationGoal::new(maximum.clone(), maximum.clone(), target, 1_000).unwrap();
    let grid = Traversability4x4::new(
        100,
        100,
        cells([TraversabilityCell::Free; 16]),
        maximum.clone(),
        maximum.clone(),
        0,
        0,
        u64::MAX,
        maximum.clone(),
        validity(),
    )
    .unwrap();
    let time = NavigationTime::new(maximum, 500).unwrap();

    let RouteDecision::Route(route) = route_grid4(&pose, &goal, &grid, &time).unwrap() else {
        panic!("maximal valid identities must still produce a route")
    };
    assert_eq!(route.identity().len(), 70);
    assert_eq!(route.planning_input_identity().len(), 251);
    assert!(time_parameterize(&route, &time, 100, 50, 30_000_000).is_ok());
}

#[test]
fn reach_frame_obeys_the_same_source_identity_bound() {
    assert!(conduit_robotics::NavigationIdentity64::new("x".repeat(65)).is_err());
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
    assert_eq!(decoded_route.identity(), route.identity());
    assert_eq!(decoded_route.waypoints(), route.waypoints());

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

#[test]
fn navigation_input_codecs_round_trip_exact_finite_values() {
    let pose = pose(50, 50, 0);
    let goal = goal(250, 250, 0);
    let mut values = [TraversabilityCell::Free; 16];
    values[1] = TraversabilityCell::Blocked;
    let traversability = grid_with_cells(values);
    let time = time(500);

    assert_eq!(
        conduit_semantic_catalog::decode_navigation_pose(
            &conduit_semantic_catalog::encode_navigation_pose(&pose, 0, 0).unwrap()
        )
        .unwrap(),
        pose
    );
    assert_eq!(
        conduit_semantic_catalog::decode_navigation_goal(
            &conduit_semantic_catalog::encode_navigation_goal(&goal).unwrap()
        )
        .unwrap(),
        goal
    );
    assert_eq!(
        conduit_semantic_catalog::decode_navigation_traversability(
            &conduit_semantic_catalog::encode_navigation_traversability(&traversability).unwrap()
        )
        .unwrap(),
        traversability
    );
    assert_eq!(
        conduit_semantic_catalog::decode_navigation_time(
            &conduit_semantic_catalog::encode_navigation_time(&time).unwrap()
        )
        .unwrap(),
        time
    );
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_presentation::install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    install_robotics_structured_catalogs(&mut startup, &mut profile).unwrap();
    install_navigation_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn host(selector_definitions: Vec<conduit_plot::KindProjection>) -> HostAdvertisement {
    let mut capabilities = navigation_kind_contracts()
        .into_iter()
        .map(|(kind, inputs, outputs)| {
            offer(
                kind,
                inputs,
                outputs,
                KindIdentity::from(NAVIGATION_REVISION),
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
                .map(|field| FrontStartupParameter {
                    name: field.key,
                    value_type: conduit_core::kind_id("value/text"),
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
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}

fn offer(
    kind: conduit_core::KindId,
    inputs: Vec<conduit_core::PortDescriptor>,
    outputs: Vec<conduit_core::PortDescriptor>,
    revision: KindIdentity,
    startup_parameters: Vec<FrontStartupParameter>,
) -> CapabilityOffer {
    conduit_core::capability_offer_from_parts! {
        semantic_contract: Default::default(),
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
        host_calls: vec![],
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
    NavigationTime::new(id("clock/fixture"), now_ms).unwrap()
}

fn pose(x_mm: i32, y_mm: i32, heading_microdegrees: i32) -> NavigationPose {
    NavigationPose::new(
        id("clock/fixture"),
        id("map/local"),
        heading_microdegrees,
        7,
        id("pose/fixture"),
        validity(),
        x_mm,
        y_mm,
    )
    .unwrap()
}

fn goal(x_mm: i32, y_mm: i32, heading_microdegrees: i32) -> NavigationGoal {
    let target = GoalTarget::reach(
        id("map/local"),
        heading_microdegrees,
        2_000_000,
        5,
        x_mm,
        y_mm,
    )
    .unwrap();
    NavigationGoal::new(id("clock/fixture"), id("goal/B"), target, 1_000).unwrap()
}

fn grid(cell: TraversabilityCell) -> Traversability4x4 {
    grid_with_cells([cell; 16])
}

fn id(value: &str) -> conduit_robotics::NavigationIdentity64 {
    conduit_robotics::NavigationIdentity64::new(value.into()).unwrap()
}

fn validity() -> Validity {
    Validity::new(400, 600).unwrap()
}

fn cells(values: [TraversabilityCell; 16]) -> conduit_robotics::NavigationTraversabilityCells {
    conduit_robotics::NavigationTraversabilityCells::new(values).unwrap()
}

fn grid_with_cells(values: [TraversabilityCell; 16]) -> Traversability4x4 {
    Traversability4x4::new(
        100,
        100,
        cells(values),
        id("clock/fixture"),
        id("map/local"),
        0,
        0,
        11,
        id("grid/fixture"),
        validity(),
    )
    .unwrap()
}
