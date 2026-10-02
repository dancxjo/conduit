//! Native Conduitese navigation types exposed through the semantic catalog.

use alloc::{vec, vec::Vec};
use conduit_core::StructuredInfoType;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_robotics::{
    NavigationBoundedMotionIntent, NavigationControlDecision, NavigationGoal, NavigationPose,
    NavigationRoute, NavigationRouteDecision, NavigationTime, NavigationTrajectory,
    NavigationTraversability4x4, NavigationValidity,
};

pub const NAVIGATION_GOAL_TYPE: &str = "NavigationGoal";
pub const NAVIGATION_TIME_TYPE: &str = "NavigationTime";
pub const NAVIGATION_POSE_TYPE: &str = "NavigationPose";
pub const NAVIGATION_TRAVERSABILITY_TYPE: &str = "NavigationTraversability4x4";
pub const NAVIGATION_ROUTE_DECISION_TYPE: &str = "NavigationRouteDecision";
pub const NAVIGATION_ROUTE_TYPE: &str = "NavigationRoute";
pub const NAVIGATION_TRAJECTORY_TYPE: &str = "NavigationTrajectory";
pub const NAVIGATION_CONTROL_TYPE: &str = "NavigationControl";
pub const NAVIGATION_BOUNDED_MOTION_INTENT_TYPE: &str = "NavigationBoundedMotionIntent";
pub const NAVIGATION_GRID_CELLS: u16 = 16;
pub const NAVIGATION_MAXIMUM_WAYPOINTS: usize = 4;
pub const NAVIGATION_MAXIMUM_SEGMENTS: usize = 3;
pub const NAVIGATION_MAXIMUM_SOURCE_IDENTITY_BYTES: usize = 64;
pub const NAVIGATION_MAXIMUM_ROUTE_IDENTITY_BYTES: usize = 70;
pub const NAVIGATION_MAXIMUM_PLANNING_INPUT_IDENTITY_BYTES: usize = 251;

fn native<T: NativeRustBinding>() -> StructuredInfoType {
    T::semantic_type().expect("checked native navigation Type")
}

pub fn navigation_validity_type() -> StructuredInfoType {
    native::<NavigationValidity>()
}
pub fn navigation_time_type() -> StructuredInfoType {
    native::<NavigationTime>()
}
pub fn navigation_goal_type() -> StructuredInfoType {
    native::<NavigationGoal>()
}
pub fn navigation_pose_type() -> StructuredInfoType {
    native::<NavigationPose>()
}
pub fn navigation_traversability_type() -> StructuredInfoType {
    native::<NavigationTraversability4x4>()
}
pub fn navigation_route_type() -> StructuredInfoType {
    native::<NavigationRoute>()
}
pub fn navigation_route_decision_type() -> StructuredInfoType {
    native::<NavigationRouteDecision>()
}
pub fn navigation_trajectory_type() -> StructuredInfoType {
    native::<NavigationTrajectory>()
}
pub fn navigation_control_type() -> StructuredInfoType {
    native::<NavigationControlDecision>()
}
pub fn navigation_bounded_motion_intent_type() -> StructuredInfoType {
    native::<NavigationBoundedMotionIntent>()
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
        (
            NAVIGATION_BOUNDED_MOTION_INTENT_TYPE,
            navigation_bounded_motion_intent_type(),
        ),
    ]
}
