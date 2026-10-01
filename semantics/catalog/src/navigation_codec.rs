//! Canonical structured-value codec for portable navigation operations.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    Quantity, QuantityUnit, StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_presentation::{path2_value, point2_value};

use crate::navigation_codec_support::*;
use crate::{
    decode_navigation_goal, decode_navigation_time, local_control, navigation_control_type,
    navigation_pose_type, navigation_route_decision_type, navigation_route_type,
    navigation_trajectory_type, navigation_traversability_type, route_grid4, ControlDecision,
    NavigationPose, NavigationRefusal, NavigationRoute, NavigationTrajectory, RouteDecision,
    TrajectorySegment, Traversability4x4, TraversabilityCell, Validity, Waypoint,
};
use conduit_robotics::{motion_request_value, twist_interval_value, ROBOTICS_BODY_FRAME};
use conduit_robotics::{
    NavigationControlDecisionMotion, NavigationIdentity64, NavigationPlanningInputIdentity,
    NavigationRouteDecisionRoute, NavigationRouteIdentity, NavigationTrajectorySegments,
    NavigationTraversabilityCells, NavigationWaypoints,
};

fn identity64(value: String) -> Result<NavigationIdentity64, NavigationCodecError> {
    NavigationIdentity64::new(value).map_err(|_| NavigationCodecError::Malformed)
}

pub fn decode_navigation_pose(encoded: &[u8]) -> Result<NavigationPose, NavigationCodecError> {
    let value = exact(encoded, &navigation_pose_type())?;
    let observation = record_field(&value, "observation")?;
    let pose = record_field(observation, "pose")?;
    let position = record_field(pose, "position")?;
    let sample = record_field(observation, "sample")?;
    let validity = validity(record_field(&value, "validity")?)?;
    let observed_at_ms = u64_quantity(
        record_field(sample, "sample_time_since_boot")?,
        QuantityUnit::Millisecond,
    )?;
    let validity_value =
        Validity::new(observed_at_ms, validity.1).map_err(|_| NavigationCodecError::Malformed)?;
    NavigationPose::new(
        identity64(validity.0)?,
        identity64(text(record_field(position, "frame")?)?)?,
        i32::try_from(quantity(
            record_field(pose, "heading")?,
            QuantityUnit::Microdegree,
        )?)
        .map_err(|_| NavigationCodecError::InexactQuantity)?,
        count(record_field(sample, "sample_sequence")?)?,
        identity64(text(record_field(sample, "source_identity")?)?)?,
        validity_value,
        i32::try_from(quantity(
            record_field(position, "x")?,
            QuantityUnit::Millimeter,
        )?)
        .map_err(|_| NavigationCodecError::InexactQuantity)?,
        i32::try_from(quantity(
            record_field(position, "y")?,
            QuantityUnit::Millimeter,
        )?)
        .map_err(|_| NavigationCodecError::InexactQuantity)?,
    )
    .map_err(|_| NavigationCodecError::Malformed)
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
    let cells: [TraversabilityCell; 16] = cells
        .try_into()
        .map_err(|_| NavigationCodecError::Malformed)?;
    let cells =
        NavigationTraversabilityCells::new(cells).map_err(|_| NavigationCodecError::Malformed)?;
    let validity_value = Validity::new(
        u64_quantity(
            record_field(sample, "sample_time_since_boot")?,
            QuantityUnit::Millisecond,
        )?,
        valid.1,
    )
    .map_err(|_| NavigationCodecError::Malformed)?;
    Traversability4x4::new(
        u32_quantity(record_field(extent, "height")?, QuantityUnit::Millimeter)?,
        u32_quantity(record_field(extent, "width")?, QuantityUnit::Millimeter)?,
        cells,
        identity64(valid.0)?,
        identity64(text(record_field(&value, "frame")?)?)?,
        i32_quantity(record_field(origin, "x")?, QuantityUnit::Millimeter)?,
        i32_quantity(record_field(origin, "y")?, QuantityUnit::Millimeter)?,
        count(record_field(sample, "sample_sequence")?)?,
        identity64(text(record_field(sample, "source_identity")?)?)?,
        validity_value,
    )
    .map_err(|_| NavigationCodecError::Malformed)
}

pub fn encode_navigation_pose(
    value: &NavigationPose,
    position_uncertainty_mm: u32,
    heading_uncertainty_microdegrees: u32,
) -> Result<Vec<u8>, NavigationCodecError> {
    let ty = navigation_pose_type();
    let observation_ty = field_type(&ty, "observation")?;
    let pose_ty = field_type(&observation_ty, "pose")?;
    let sample_ty = field_type(&observation_ty, "sample")?;
    let pose = record_value(
        pose_ty,
        vec![
            (
                "heading",
                quantity_value(value.heading_microdegrees.into(), QuantityUnit::Microdegree)?,
            ),
            (
                "position",
                point2_value(
                    &value.frame,
                    Quantity::new(value.x_mm.into(), QuantityUnit::Millimeter),
                    Quantity::new(value.y_mm.into(), QuantityUnit::Millimeter),
                )
                .map_err(|_| NavigationCodecError::Malformed)?,
            ),
        ],
    )?;
    let sample = sample_value(
        sample_ty,
        &value.source_identity,
        value.sample_sequence,
        value.validity.observed_at_ms,
    )?;
    let observation = record_value(
        observation_ty,
        vec![
            (
                "heading_uncertainty",
                quantity_value(
                    heading_uncertainty_microdegrees.into(),
                    QuantityUnit::Microdegree,
                )?,
            ),
            ("pose", pose),
            (
                "position_uncertainty",
                quantity_value(position_uncertainty_mm.into(), QuantityUnit::Millimeter)?,
            ),
            ("sample", sample),
        ],
    )?;
    let validity = validity_value(&ty, &value.clock_identity, value.validity.valid_until_ms)?;
    Ok(record_value(
        ty,
        vec![("observation", observation), ("validity", validity)],
    )?
    .canonical_bytes()?)
}

pub fn encode_navigation_traversability(
    value: &Traversability4x4,
) -> Result<Vec<u8>, NavigationCodecError> {
    let ty = navigation_traversability_type();
    let cells_ty = field_type(&ty, "cells")?;
    let StructuredInfoTypeShape::Collection { element, .. } = cells_ty.shape() else {
        return Err(NavigationCodecError::Malformed);
    };
    let cells = value
        .cells
        .iter()
        .map(|cell| {
            let tag = match cell {
                TraversabilityCell::Free => "free",
                TraversabilityCell::Blocked => "blocked",
                TraversabilityCell::Unknown => "unknown",
            };
            let unit = variant_type(element, tag)?;
            Ok(StructuredInfoValue::variant(
                element.clone(),
                tag,
                StructuredInfoValue::leaf(unit, Vec::new())?,
            )?)
        })
        .collect::<Result<Vec<_>, NavigationCodecError>>()?;
    let cells = StructuredInfoValue::collection(cells_ty, cells)?;
    let extent_ty = field_type(&ty, "cell_extent")?;
    let extent = record_value(
        extent_ty,
        vec![
            (
                "height",
                quantity_value(value.cell_height_mm.into(), QuantityUnit::Millimeter)?,
            ),
            (
                "width",
                quantity_value(value.cell_width_mm.into(), QuantityUnit::Millimeter)?,
            ),
        ],
    )?;
    let sample_ty = field_type(&ty, "sample")?;
    let sample = sample_value(
        sample_ty,
        &value.source_identity,
        value.sample_sequence,
        value.validity.observed_at_ms,
    )?;
    let validity = validity_value(&ty, &value.clock_identity, value.validity.valid_until_ms)?;
    Ok(record_value(
        ty,
        vec![
            ("cells", cells),
            ("cell_extent", extent),
            ("frame", text_value(&value.frame)?),
            (
                "origin",
                point2_value(
                    &value.frame,
                    Quantity::new(value.origin_x_mm.into(), QuantityUnit::Millimeter),
                    Quantity::new(value.origin_y_mm.into(), QuantityUnit::Millimeter),
                )
                .map_err(|_| NavigationCodecError::Malformed)?,
            ),
            ("sample", sample),
            ("validity", validity),
        ],
    )?
    .canonical_bytes()?)
}

fn sample_value(
    ty: conduit_core::StructuredInfoType,
    source_identity: &str,
    sample_sequence: u64,
    observed_at_ms: u64,
) -> Result<StructuredInfoValue, NavigationCodecError> {
    record_value(
        ty,
        vec![
            ("sample_sequence", count_value(sample_sequence)?),
            (
                "sample_time_since_boot",
                quantity_value(
                    observed_at_ms
                        .try_into()
                        .map_err(|_| NavigationCodecError::InexactQuantity)?,
                    QuantityUnit::Millisecond,
                )?,
            ),
            ("source_identity", text_value(source_identity)?),
        ],
    )
}

fn validity_value(
    parent: &conduit_core::StructuredInfoType,
    clock_identity: &str,
    valid_until_ms: u64,
) -> Result<StructuredInfoValue, NavigationCodecError> {
    record_value(
        field_type(parent, "validity")?,
        vec![
            ("clock_identity", text_value(clock_identity)?),
            (
                "valid_until",
                quantity_value(
                    valid_until_ms
                        .try_into()
                        .map_err(|_| NavigationCodecError::InexactQuantity)?,
                    QuantityUnit::Millisecond,
                )?,
            ),
        ],
    )
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
    let waypoints = conduit_form::rust_binding::BoundedSequence::try_from_iter(waypoints)
        .map_err(|_| NavigationCodecError::Malformed)?;
    let waypoints =
        NavigationWaypoints::new(waypoints).map_err(|_| NavigationCodecError::Malformed)?;
    NavigationRoute::new(
        identity64(frame.ok_or(NavigationCodecError::Malformed)?)?,
        identity64(text(record_field(&value, "goal_identity")?)?)?,
        u32_quantity(
            record_field(&value, "heading_tolerance")?,
            QuantityUnit::Microdegree,
        )?,
        NavigationRouteIdentity::new(text(record_field(&value, "route_identity")?)?)
            .map_err(|_| NavigationCodecError::Malformed)?,
        identity64(text(record_field(&value, "planner_identity")?)?)?,
        NavigationPlanningInputIdentity::new(text(record_field(
            &value,
            "planning_input_identity",
        )?)?)
        .map_err(|_| NavigationCodecError::Malformed)?,
        u32_quantity(
            record_field(&value, "position_tolerance")?,
            QuantityUnit::Millimeter,
        )?,
        i32_quantity(
            record_field(&value, "target_heading")?,
            QuantityUnit::Microdegree,
        )?,
        waypoints,
    )
    .map_err(|_| NavigationCodecError::Malformed)
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
    let segments = conduit_form::rust_binding::BoundedSequence::try_from_iter(segments)
        .map_err(|_| NavigationCodecError::Malformed)?;
    let segments =
        NavigationTrajectorySegments::new(segments).map_err(|_| NavigationCodecError::Malformed)?;
    NavigationTrajectory::new(
        identity64(text(record_field(&value, "clock_identity")?)?)?,
        identity64(frame.ok_or(NavigationCodecError::Malformed)?)?,
        identity64(text(record_field(&value, "goal_identity")?)?)?,
        u32_quantity(
            record_field(&value, "heading_tolerance")?,
            QuantityUnit::Microdegree,
        )?,
        u32_quantity(
            record_field(&value, "position_tolerance")?,
            QuantityUnit::Millimeter,
        )?,
        NavigationRouteIdentity::new(text(record_field(&value, "route_identity")?)?)
            .map_err(|_| NavigationCodecError::Malformed)?,
        segments,
        i32_quantity(
            record_field(&value, "target_heading")?,
            QuantityUnit::Microdegree,
        )?,
        u64_quantity(
            record_field(&value, "valid_until")?,
            QuantityUnit::Millisecond,
        )?,
    )
    .map_err(|_| NavigationCodecError::Malformed)
}

pub fn encode_route_decision(decision: &RouteDecision) -> Result<Vec<u8>, NavigationCodecError> {
    let (tag, payload) = match decision {
        RouteDecision::Route(route) => ("route", decision_route_value(route)?),
        RouteDecision::Hold(payload) => ("hold", refusal_value(payload.goal_identity(), "hold")?),
        RouteDecision::NoPath(payload) => (
            "no_path",
            refusal_value(payload.goal_identity(), "no-path")?,
        ),
        RouteDecision::GoalInvalid(payload) => (
            "goal_invalid",
            refusal_value(payload.goal_identity(), "goal-invalid")?,
        ),
        RouteDecision::ObstacleDataUnavailable(payload) => (
            "obstacle_data_unavailable",
            refusal_value(payload.goal_identity(), "obstacle-data-unavailable")?,
        ),
        RouteDecision::PoseStale(payload) => (
            "pose_stale",
            refusal_value(payload.goal_identity(), "pose-stale")?,
        ),
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
        ControlDecision::Arrived(payload) => ("arrived", text_value(payload.goal_identity())?),
        ControlDecision::Hold(payload) => ("hold", text_value(payload.goal_identity())?),
        ControlDecision::PoseStale(payload) => ("pose_stale", text_value(payload.goal_identity())?),
        ControlDecision::TrajectoryExpired(payload) => {
            ("trajectory_expired", text_value(payload.goal_identity())?)
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

fn decision_route_value(
    route: &NavigationRouteDecisionRoute,
) -> Result<StructuredInfoValue, NavigationCodecError> {
    let points = route
        .waypoints()
        .iter()
        .map(|point| {
            point2_value(
                route.frame(),
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
        StructuredInfoValue::variant(course_ty, count_tag(route.waypoints().len())?, path)?;
    record_value(
        ty,
        vec![
            ("goal_identity", text_value(route.goal_identity())?),
            (
                "heading_tolerance",
                quantity_value(
                    (*route.heading_tolerance_microdegrees()).into(),
                    QuantityUnit::Microdegree,
                )?,
            ),
            ("planner_identity", text_value(route.planner_identity())?),
            (
                "planning_input_identity",
                text_value(route.planning_input_identity())?,
            ),
            (
                "position_tolerance",
                quantity_value(
                    (*route.position_tolerance_mm()).into(),
                    QuantityUnit::Millimeter,
                )?,
            ),
            ("route_identity", text_value(route.identity())?),
            (
                "target_heading",
                quantity_value(
                    (*route.target_heading_microdegrees()).into(),
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

fn motion_value(
    intent: &NavigationControlDecisionMotion,
) -> Result<StructuredInfoValue, NavigationCodecError> {
    let twist = twist_interval_value(
        ROBOTICS_BODY_FRAME,
        Quantity::new((*intent.interval_ms()).into(), QuantityUnit::Millisecond),
        Quantity::new((*intent.linear_mm()).into(), QuantityUnit::Millimeter),
        Quantity::new(0, QuantityUnit::Millimeter),
        Quantity::new(
            (*intent.angular_microdegrees()).into(),
            QuantityUnit::Microdegree,
        ),
    )
    .map_err(|_| NavigationCodecError::Robotics)?;
    motion_request_value(
        &format!("navigation/{}", intent.goal_identity()),
        Quantity::new((*intent.ttl_ms()).into(), QuantityUnit::Millisecond),
        twist,
    )
    .map_err(|_| NavigationCodecError::Robotics)
}
