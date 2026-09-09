//! Finite host-neutral navigation Info contracts.
//!
//! These values describe goals, current spatial evidence, routes, trajectories,
//! and controller dispositions. They contain no Host, Conduit Plan, motor, or
//! device-protocol identity.

use alloc::{vec, vec::Vec};
use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType, StructuredVariantCase};
use conduit_presentation::{extent2_type, path2_type, point2_type, robotics_pose2_type};

use crate::{
    robotics_motion_request_type, robotics_pose_sample_type, robotics_sample_context_type,
};

pub const NAVIGATION_GOAL_TYPE: &str = "NavigationGoal";
pub const NAVIGATION_TIME_TYPE: &str = "NavigationTime";
pub const NAVIGATION_POSE_TYPE: &str = "NavigationPose";
pub const NAVIGATION_TRAVERSABILITY_TYPE: &str = "NavigationTraversability4x4";
pub const NAVIGATION_ROUTE_DECISION_TYPE: &str = "NavigationRouteDecision";
pub const NAVIGATION_ROUTE_TYPE: &str = "NavigationRoute";
pub const NAVIGATION_TRAJECTORY_TYPE: &str = "NavigationTrajectory";
pub const NAVIGATION_CONTROL_TYPE: &str = "NavigationControl";
pub const NAVIGATION_GRID_CELLS: u16 = 16;
pub const NAVIGATION_MAXIMUM_WAYPOINTS: usize = 4;
pub const NAVIGATION_MAXIMUM_SEGMENTS: usize = 3;
pub const NAVIGATION_MAXIMUM_IDENTITY_BYTES: usize = 64;

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed navigation leaf")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

fn quantity_type() -> StructuredInfoType {
    leaf(conduit_core::QUANTITY_INFO_ID)
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed navigation field")
}

fn case(name: &str, payload: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload).expect("reviewed navigation case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed navigation record")
}

pub fn navigation_validity_type() -> StructuredInfoType {
    record(
        "navigation/validity@1",
        vec![
            field("clock_identity", text_type()),
            field("valid_until", quantity_type()),
        ],
    )
}

pub fn navigation_time_type() -> StructuredInfoType {
    record(
        "navigation/time@1",
        vec![
            field("clock_identity", text_type()),
            field("now", quantity_type()),
        ],
    )
}

fn navigation_reach_goal_type() -> StructuredInfoType {
    record(
        "navigation/reach-goal@1",
        vec![
            field("heading_tolerance", quantity_type()),
            field("pose", robotics_pose2_type()),
            field("position_tolerance", quantity_type()),
        ],
    )
}

pub fn navigation_goal_type() -> StructuredInfoType {
    let target = StructuredInfoType::variant(
        kind_id("navigation/goal-target@1"),
        vec![
            case("hold", unit_type()),
            case("reach", navigation_reach_goal_type()),
        ],
    )
    .expect("reviewed navigation goal target");
    record(
        "navigation/goal@1",
        vec![
            field("goal_identity", text_type()),
            field("target", target),
            field("validity", navigation_validity_type()),
        ],
    )
}

pub fn navigation_pose_type() -> StructuredInfoType {
    record(
        "navigation/pose@1",
        vec![
            field("observation", robotics_pose_sample_type()),
            field("validity", navigation_validity_type()),
        ],
    )
}

fn navigation_cell_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("navigation/traversability-cell@1"),
        vec![
            case("blocked", unit_type()),
            case("free", unit_type()),
            case("unknown", unit_type()),
        ],
    )
    .expect("reviewed navigation cell")
}

pub fn navigation_traversability_type() -> StructuredInfoType {
    record(
        "navigation/traversability-grid4x4@1",
        vec![
            field(
                "cells",
                StructuredInfoType::collection(navigation_cell_type(), Some(NAVIGATION_GRID_CELLS))
                    .expect("sixteen cells are bounded"),
            ),
            field("cell_extent", extent2_type()),
            field("frame", text_type()),
            field("origin", point2_type()),
            field("sample", robotics_sample_context_type()),
            field("validity", navigation_validity_type()),
        ],
    )
}

fn route_course_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("navigation/route-course@1"),
        (1..=NAVIGATION_MAXIMUM_WAYPOINTS)
            .map(|count| {
                case(
                    match count {
                        1 => "one",
                        2 => "two",
                        3 => "three",
                        _ => "four",
                    },
                    path2_type(count as u16).expect("reviewed route bound"),
                )
            })
            .collect(),
    )
    .expect("reviewed route course")
}

pub fn navigation_route_type() -> StructuredInfoType {
    record(
        "navigation/route@1",
        vec![
            field("goal_identity", text_type()),
            field("heading_tolerance", quantity_type()),
            field("planner_identity", text_type()),
            field("planning_input_identity", text_type()),
            field("position_tolerance", quantity_type()),
            field("route_identity", text_type()),
            field("target_heading", quantity_type()),
            field("waypoints", route_course_type()),
        ],
    )
}

fn route_refusal_type() -> StructuredInfoType {
    record(
        "navigation/route-refusal@1",
        vec![
            field("goal_identity", text_type()),
            field("reason", text_type()),
        ],
    )
}

pub fn navigation_route_decision_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("navigation/route-decision@1"),
        vec![
            case("goal_invalid", route_refusal_type()),
            case("hold", route_refusal_type()),
            case("no_path", route_refusal_type()),
            case("obstacle_data_unavailable", route_refusal_type()),
            case("pose_stale", route_refusal_type()),
            case("route", navigation_route_type()),
        ],
    )
    .expect("reviewed route decision")
}

fn trajectory_segment_type() -> StructuredInfoType {
    record(
        "navigation/trajectory-segment@1",
        vec![
            field("interval", quantity_type()),
            field("maximum_angular_step", quantity_type()),
            field("maximum_linear_step", quantity_type()),
            field("target", point2_type()),
        ],
    )
}

pub fn navigation_trajectory_type() -> StructuredInfoType {
    record(
        "navigation/trajectory@1",
        vec![
            field("clock_identity", text_type()),
            field("goal_identity", text_type()),
            field("heading_tolerance", quantity_type()),
            field("position_tolerance", quantity_type()),
            field("valid_until", quantity_type()),
            field("route_identity", text_type()),
            field("target_heading", quantity_type()),
            field(
                "segments",
                StructuredInfoType::variant(
                    kind_id("navigation/trajectory-segments@1"),
                    (1..=NAVIGATION_MAXIMUM_SEGMENTS)
                        .map(|count| {
                            case(
                                match count {
                                    1 => "one",
                                    2 => "two",
                                    _ => "three",
                                },
                                StructuredInfoType::collection(
                                    trajectory_segment_type(),
                                    Some(count as u16),
                                )
                                .expect("reviewed trajectory bound"),
                            )
                        })
                        .collect(),
                )
                .expect("reviewed trajectory segments"),
            ),
        ],
    )
}

pub fn navigation_control_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("navigation/control@1"),
        vec![
            case("arrived", text_type()),
            case("hold", text_type()),
            case("motion", robotics_motion_request_type()),
            case("pose_stale", text_type()),
            case("trajectory_expired", text_type()),
        ],
    )
    .expect("reviewed navigation control")
}

pub fn navigation_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (NAVIGATION_TIME_TYPE, navigation_time_type()),
        (NAVIGATION_GOAL_TYPE, navigation_goal_type()),
        (NAVIGATION_POSE_TYPE, navigation_pose_type()),
        (
            NAVIGATION_TRAVERSABILITY_TYPE,
            navigation_traversability_type(),
        ),
        (
            NAVIGATION_ROUTE_DECISION_TYPE,
            navigation_route_decision_type(),
        ),
        (NAVIGATION_ROUTE_TYPE, navigation_route_type()),
        (NAVIGATION_TRAJECTORY_TYPE, navigation_trajectory_type()),
        (NAVIGATION_CONTROL_TYPE, navigation_control_type()),
    ]
}
