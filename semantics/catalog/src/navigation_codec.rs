//! Canonical structured-value codec for portable navigation operations.

use alloc::{format, vec, vec::Vec};
use conduit_core::{
    Quantity, QuantityUnit, StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_presentation::{path2_value, point2_value};

use crate::navigation_codec_support::*;
use crate::{
    decode_navigation_goal, decode_navigation_time, local_control, motion_request_value,
    navigation_control_type, navigation_pose_type, navigation_route_decision_type,
    navigation_route_type, navigation_trajectory_type, navigation_traversability_type, route_grid4,
    time_parameterize, twist_interval_value, BoundedMotionIntent, ControlDecision, NavigationPose,
    NavigationRefusal, NavigationRoute, NavigationTrajectory, RouteDecision, TrajectorySegment,
    Traversability4x4, TraversabilityCell, Validity, Waypoint, ROBOTICS_BODY_FRAME,
};

pub fn decode_navigation_pose(encoded: &[u8]) -> Result<NavigationPose, NavigationCodecError> {
    let value = exact(encoded, &navigation_pose_type())?;
    let observation = record_field(&value, "observation")?;
    let pose = record_field(observation, "pose")?;
    let position = record_field(pose, "position")?;
    let sample = record_field(observation, "sample")?;
    let validity = validity(record_field(&value, "validity")?)?;
    Ok(NavigationPose {
        source_identity: text(record_field(sample, "source_identity")?)?,
        sample_sequence: count(record_field(sample, "sample_sequence")?)?,
        clock_identity: validity.0,
        frame: text(record_field(position, "frame")?)?,
        x_mm: i32::try_from(quantity(
            record_field(position, "x")?,
            QuantityUnit::Millimeter,
        )?)
        .map_err(|_| NavigationCodecError::InexactQuantity)?,
        y_mm: i32::try_from(quantity(
            record_field(position, "y")?,
            QuantityUnit::Millimeter,
        )?)
        .map_err(|_| NavigationCodecError::InexactQuantity)?,
        heading_microdegrees: i32::try_from(quantity(
            record_field(pose, "heading")?,
            QuantityUnit::Microdegree,
        )?)
        .map_err(|_| NavigationCodecError::InexactQuantity)?,
        validity: Validity {
            observed_at_ms: u64_quantity(
                record_field(sample, "sample_time_since_boot")?,
                QuantityUnit::Millisecond,
            )?,
            valid_until_ms: validity.1,
        },
    })
}

pub fn decode_navigation_traversability(
    encoded: &[u8],
) -> Result<Traversability4x4, NavigationCodecError> {
    let value = exact(encoded, &navigation_traversability_type())?;
    let origin = record_field(&value, "origin")?;
    let extent = record_field(&value, "cell_extent")?;
    let sample = record_field(&value, "sample")?;
    let valid = validity(record_field(&value, "validity")?)?;
    let StructuredInfoValueShape::Collection(values) = record_field(&value, "cells")?.shape()
    else {
        return Err(NavigationCodecError::Malformed);
    };
    let cells: Vec<_> = values
        .iter()
        .map(|cell| match cell.shape() {
            StructuredInfoValueShape::Variant { tag: "free", .. } => Ok(TraversabilityCell::Free),
            StructuredInfoValueShape::Variant { tag: "blocked", .. } => {
                Ok(TraversabilityCell::Blocked)
            }
            StructuredInfoValueShape::Variant { tag: "unknown", .. } => {
                Ok(TraversabilityCell::Unknown)
            }
            _ => Err(NavigationCodecError::Malformed),
        })
        .collect::<Result<_, _>>()?;
    Ok(Traversability4x4 {
        source_identity: text(record_field(sample, "source_identity")?)?,
        sample_sequence: count(record_field(sample, "sample_sequence")?)?,
        clock_identity: valid.0,
        frame: text(record_field(&value, "frame")?)?,
        origin_x_mm: i32_quantity(record_field(origin, "x")?, QuantityUnit::Millimeter)?,
        origin_y_mm: i32_quantity(record_field(origin, "y")?, QuantityUnit::Millimeter)?,
        cell_width_mm: u32_quantity(record_field(extent, "width")?, QuantityUnit::Millimeter)?,
        cell_height_mm: u32_quantity(record_field(extent, "height")?, QuantityUnit::Millimeter)?,
        validity: Validity {
            observed_at_ms: u64_quantity(
                record_field(sample, "sample_time_since_boot")?,
                QuantityUnit::Millisecond,
            )?,
            valid_until_ms: valid.1,
        },
        cells: cells
            .try_into()
            .map_err(|_| NavigationCodecError::Malformed)?,
    })
}

pub fn decode_navigation_route(encoded: &[u8]) -> Result<NavigationRoute, NavigationCodecError> {
    let value = exact(encoded, &navigation_route_type())?;
    let StructuredInfoValueShape::Variant { payload, .. } =
        record_field(&value, "waypoints")?.shape()
    else {
        return Err(NavigationCodecError::Malformed);
    };
    let StructuredInfoValueShape::Collection(points) = record_field(payload, "points")?.shape()
    else {
        return Err(NavigationCodecError::Malformed);
    };
    let mut frame = None;
    let waypoints = points
        .iter()
        .map(|point| {
            let current = text(record_field(point, "frame")?)?;
            if frame.as_ref().is_some_and(|expected| expected != &current) {
                return Err(NavigationCodecError::Malformed);
            }
            frame = Some(current);
            Ok(Waypoint {
                x_mm: i32_quantity(record_field(point, "x")?, QuantityUnit::Millimeter)?,
                y_mm: i32_quantity(record_field(point, "y")?, QuantityUnit::Millimeter)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NavigationRoute {
        identity: text(record_field(&value, "route_identity")?)?,
        goal_identity: text(record_field(&value, "goal_identity")?)?,
        planner_identity: text(record_field(&value, "planner_identity")?)?,
        planning_input_identity: text(record_field(&value, "planning_input_identity")?)?,
        frame: frame.ok_or(NavigationCodecError::Malformed)?,
        target_heading_microdegrees: i32_quantity(
            record_field(&value, "target_heading")?,
            QuantityUnit::Microdegree,
        )?,
        position_tolerance_mm: u32_quantity(
            record_field(&value, "position_tolerance")?,
            QuantityUnit::Millimeter,
        )?,
        heading_tolerance_microdegrees: u32_quantity(
            record_field(&value, "heading_tolerance")?,
            QuantityUnit::Microdegree,
        )?,
        waypoints,
    })
}

pub fn decode_navigation_trajectory(
    encoded: &[u8],
) -> Result<NavigationTrajectory, NavigationCodecError> {
    let value = exact(encoded, &navigation_trajectory_type())?;
    let StructuredInfoValueShape::Variant { payload, .. } =
        record_field(&value, "segments")?.shape()
    else {
        return Err(NavigationCodecError::Malformed);
    };
    let StructuredInfoValueShape::Collection(values) = payload.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    let mut frame = None;
    let segments = values
        .iter()
        .map(|segment| {
            let target = record_field(segment, "target")?;
            let current = text(record_field(target, "frame")?)?;
            if frame.as_ref().is_some_and(|expected| expected != &current) {
                return Err(NavigationCodecError::Malformed);
            }
            frame = Some(current);
            Ok(TrajectorySegment {
                target: Waypoint {
                    x_mm: i32_quantity(record_field(target, "x")?, QuantityUnit::Millimeter)?,
                    y_mm: i32_quantity(record_field(target, "y")?, QuantityUnit::Millimeter)?,
                },
                interval_ms: u32_quantity(
                    record_field(segment, "interval")?,
                    QuantityUnit::Millisecond,
                )?,
                maximum_linear_step_mm: u32_quantity(
                    record_field(segment, "maximum_linear_step")?,
                    QuantityUnit::Millimeter,
                )?,
                maximum_angular_step_microdegrees: u32_quantity(
                    record_field(segment, "maximum_angular_step")?,
                    QuantityUnit::Microdegree,
                )?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NavigationTrajectory {
        route_identity: text(record_field(&value, "route_identity")?)?,
        goal_identity: text(record_field(&value, "goal_identity")?)?,
        clock_identity: text(record_field(&value, "clock_identity")?)?,
        target_heading_microdegrees: i32_quantity(
            record_field(&value, "target_heading")?,
            QuantityUnit::Microdegree,
        )?,
        position_tolerance_mm: u32_quantity(
            record_field(&value, "position_tolerance")?,
            QuantityUnit::Millimeter,
        )?,
        heading_tolerance_microdegrees: u32_quantity(
            record_field(&value, "heading_tolerance")?,
            QuantityUnit::Microdegree,
        )?,
        valid_until_ms: u64_quantity(
            record_field(&value, "valid_until")?,
            QuantityUnit::Millisecond,
        )?,
        frame: frame.ok_or(NavigationCodecError::Malformed)?,
        segments,
    })
}

pub fn encode_route_decision(decision: &RouteDecision) -> Result<Vec<u8>, NavigationCodecError> {
    let (tag, payload) = match decision {
        RouteDecision::Route(route) => ("route", route_value(route)?),
        RouteDecision::Hold { goal_identity } => ("hold", refusal_value(goal_identity, "hold")?),
        RouteDecision::NoPath { goal_identity } => {
            ("no_path", refusal_value(goal_identity, "no-path")?)
        }
        RouteDecision::GoalInvalid { goal_identity } => (
            "goal_invalid",
            refusal_value(goal_identity, "goal-invalid")?,
        ),
        RouteDecision::ObstacleDataUnavailable { goal_identity } => (
            "obstacle_data_unavailable",
            refusal_value(goal_identity, "obstacle-data-unavailable")?,
        ),
        RouteDecision::PoseStale { goal_identity } => {
            ("pose_stale", refusal_value(goal_identity, "pose-stale")?)
        }
    };
    Ok(
        StructuredInfoValue::variant(navigation_route_decision_type(), tag, payload)?
            .canonical_bytes()?,
    )
}

pub fn encode_trajectory(value: &NavigationTrajectory) -> Result<Vec<u8>, NavigationCodecError> {
    if value.segments.is_empty() || value.segments.len() > 3 {
        return Err(NavigationRefusal::InvalidTrajectory.into());
    }
    let ty = navigation_trajectory_type();
    let segments_ty = field_type(&ty, "segments")?;
    let tag = count_tag(value.segments.len())?;
    let collection_ty = variant_type(&segments_ty, tag)?;
    let StructuredInfoTypeShape::Collection { element, .. } = collection_ty.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    let values = value
        .segments
        .iter()
        .map(|segment| {
            record_value(
                element.clone(),
                vec![
                    (
                        "interval",
                        quantity_value(segment.interval_ms.into(), QuantityUnit::Millisecond)?,
                    ),
                    (
                        "maximum_angular_step",
                        quantity_value(
                            segment.maximum_angular_step_microdegrees.into(),
                            QuantityUnit::Microdegree,
                        )?,
                    ),
                    (
                        "maximum_linear_step",
                        quantity_value(
                            segment.maximum_linear_step_mm.into(),
                            QuantityUnit::Millimeter,
                        )?,
                    ),
                    (
                        "target",
                        point2_value(
                            &value.frame,
                            Quantity::new(segment.target.x_mm.into(), QuantityUnit::Millimeter),
                            Quantity::new(segment.target.y_mm.into(), QuantityUnit::Millimeter),
                        )
                        .map_err(|_| NavigationCodecError::Malformed)?,
                    ),
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let segments = StructuredInfoValue::variant(
        segments_ty,
        tag,
        StructuredInfoValue::collection(collection_ty, values)?,
    )?;
    Ok(record_value(
        ty,
        vec![
            ("clock_identity", text_value(&value.clock_identity)?),
            ("goal_identity", text_value(&value.goal_identity)?),
            (
                "heading_tolerance",
                quantity_value(
                    value.heading_tolerance_microdegrees.into(),
                    QuantityUnit::Microdegree,
                )?,
            ),
            (
                "position_tolerance",
                quantity_value(value.position_tolerance_mm.into(), QuantityUnit::Millimeter)?,
            ),
            ("route_identity", text_value(&value.route_identity)?),
            ("segments", segments),
            (
                "valid_until",
                quantity_value(
                    value
                        .valid_until_ms
                        .try_into()
                        .map_err(|_| NavigationCodecError::InexactQuantity)?,
                    QuantityUnit::Millisecond,
                )?,
            ),
            (
                "target_heading",
                quantity_value(
                    value.target_heading_microdegrees.into(),
                    QuantityUnit::Microdegree,
                )?,
            ),
        ],
    )?
    .canonical_bytes()?)
}

pub fn encode_control(value: &ControlDecision) -> Result<Vec<u8>, NavigationCodecError> {
    let (tag, payload) = match value {
        ControlDecision::Motion(intent) => ("motion", motion_value(intent)?),
        ControlDecision::Arrived { goal_identity } => ("arrived", text_value(goal_identity)?),
        ControlDecision::Hold { goal_identity } => ("hold", text_value(goal_identity)?),
        ControlDecision::PoseStale { goal_identity } => ("pose_stale", text_value(goal_identity)?),
        ControlDecision::TrajectoryExpired { goal_identity } => {
            ("trajectory_expired", text_value(goal_identity)?)
        }
    };
    Ok(StructuredInfoValue::variant(navigation_control_type(), tag, payload)?.canonical_bytes()?)
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
    encode_trajectory(&time_parameterize(
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

fn route_value(route: &NavigationRoute) -> Result<StructuredInfoValue, NavigationCodecError> {
    let points = route
        .waypoints
        .iter()
        .map(|point| {
            point2_value(
                &route.frame,
                Quantity::new(point.x_mm.into(), QuantityUnit::Millimeter),
                Quantity::new(point.y_mm.into(), QuantityUnit::Millimeter),
            )
            .map_err(|_| NavigationCodecError::Malformed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let path = path2_value(points).map_err(|_| NavigationCodecError::Malformed)?;
    let ty = navigation_route_type();
    let course_ty = field_type(&ty, "waypoints")?;
    let waypoints =
        StructuredInfoValue::variant(course_ty, count_tag(route.waypoints.len())?, path)?;
    record_value(
        ty,
        vec![
            ("goal_identity", text_value(&route.goal_identity)?),
            (
                "heading_tolerance",
                quantity_value(
                    route.heading_tolerance_microdegrees.into(),
                    QuantityUnit::Microdegree,
                )?,
            ),
            ("planner_identity", text_value(&route.planner_identity)?),
            (
                "planning_input_identity",
                text_value(&route.planning_input_identity)?,
            ),
            (
                "position_tolerance",
                quantity_value(route.position_tolerance_mm.into(), QuantityUnit::Millimeter)?,
            ),
            ("route_identity", text_value(&route.identity)?),
            (
                "target_heading",
                quantity_value(
                    route.target_heading_microdegrees.into(),
                    QuantityUnit::Microdegree,
                )?,
            ),
            ("waypoints", waypoints),
        ],
    )
}

fn refusal_value(goal: &str, reason: &str) -> Result<StructuredInfoValue, NavigationCodecError> {
    let decision = navigation_route_decision_type();
    let payload = variant_type(
        &decision,
        if reason == "hold" {
            "hold"
        } else {
            match reason {
                "no-path" => "no_path",
                "goal-invalid" => "goal_invalid",
                "obstacle-data-unavailable" => "obstacle_data_unavailable",
                _ => "pose_stale",
            }
        },
    )?;
    record_value(
        payload,
        vec![
            ("goal_identity", text_value(goal)?),
            ("reason", text_value(reason)?),
        ],
    )
}

fn motion_value(intent: &BoundedMotionIntent) -> Result<StructuredInfoValue, NavigationCodecError> {
    let twist = twist_interval_value(
        ROBOTICS_BODY_FRAME,
        Quantity::new(intent.interval_ms.into(), QuantityUnit::Millisecond),
        Quantity::new(intent.linear_mm.into(), QuantityUnit::Millimeter),
        Quantity::new(0, QuantityUnit::Millimeter),
        Quantity::new(
            intent.angular_microdegrees.into(),
            QuantityUnit::Microdegree,
        ),
    )
    .map_err(|_| NavigationCodecError::Robotics)?;
    motion_request_value(
        &format!("navigation/{}", intent.goal_identity),
        Quantity::new(intent.ttl_ms.into(), QuantityUnit::Millisecond),
        twist,
    )
    .map_err(|_| NavigationCodecError::Robotics)
}
