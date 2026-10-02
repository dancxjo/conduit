//! Canonical native codecs for portable navigation operations.

use crate::navigation_codec_support::{decode_native, encode_native};
use crate::{
    decode_navigation_goal, decode_navigation_time, local_control, route_grid4, ControlDecision,
    NavigationCodecError, NavigationPose, NavigationRoute, NavigationTrajectory, RouteDecision,
    Traversability4x4,
};
use alloc::vec::Vec;

pub fn decode_navigation_pose(encoded: &[u8]) -> Result<NavigationPose, NavigationCodecError> {
    decode_native(encoded)
}
pub fn decode_navigation_traversability(
    encoded: &[u8],
) -> Result<Traversability4x4, NavigationCodecError> {
    decode_native(encoded)
}

pub fn encode_navigation_pose(
    value: &NavigationPose,
    _position_uncertainty_mm: u32,
    _heading_uncertainty_microdegrees: u32,
) -> Result<Vec<u8>, NavigationCodecError> {
    encode_native(value)
}

pub fn encode_navigation_traversability(
    value: &Traversability4x4,
) -> Result<Vec<u8>, NavigationCodecError> {
    encode_native(value)
}
pub fn decode_navigation_route(encoded: &[u8]) -> Result<NavigationRoute, NavigationCodecError> {
    decode_native(encoded)
}
pub fn decode_navigation_trajectory(
    encoded: &[u8],
) -> Result<NavigationTrajectory, NavigationCodecError> {
    decode_native(encoded)
}
pub fn encode_route_decision(decision: &RouteDecision) -> Result<Vec<u8>, NavigationCodecError> {
    encode_native(decision)
}
pub fn encode_trajectory(value: &NavigationTrajectory) -> Result<Vec<u8>, NavigationCodecError> {
    encode_native(value)
}
pub fn encode_control(value: &ControlDecision) -> Result<Vec<u8>, NavigationCodecError> {
    encode_native(value)
}

pub fn execute_route_grid4(
    pose: &[u8],
    goal: &[u8],
    grid: &[u8],
    time: &[u8],
) -> Result<Vec<u8>, NavigationCodecError> {
    encode_route_decision(&route_grid4(
        &decode_navigation_pose(pose)?,
        &decode_navigation_goal(goal)?,
        &decode_navigation_traversability(grid)?,
        &decode_navigation_time(time)?,
    )?)
}

pub fn execute_time_parameterize(
    route: &[u8],
    time: &[u8],
    interval_ms: u32,
    linear_mm: u32,
    angular_microdegrees: u32,
) -> Result<Vec<u8>, NavigationCodecError> {
    encode_trajectory(&crate::navigation_realization::time_parameterize_route(
        &decode_navigation_route(route)?,
        &decode_navigation_time(time)?,
        interval_ms,
        linear_mm,
        angular_microdegrees,
    )?)
}

pub fn execute_local_control(
    pose: &[u8],
    trajectory: &[u8],
    time: &[u8],
) -> Result<Vec<u8>, NavigationCodecError> {
    encode_control(&local_control(
        &decode_navigation_pose(pose)?,
        &decode_navigation_trajectory(trajectory)?,
        &decode_navigation_time(time)?,
    )?)
}
