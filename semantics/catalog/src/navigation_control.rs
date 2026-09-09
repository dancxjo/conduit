//! Bounded local feedback control over a portable trajectory.

use crate::{
    BoundedMotionIntent, ControlDecision, NavigationPose, NavigationRefusal, NavigationTime,
    NavigationTrajectory, TrajectorySegment, NAVIGATION_MAXIMUM_SEGMENTS,
};

pub fn local_control(
    pose: &NavigationPose,
    trajectory: &NavigationTrajectory,
    time: &NavigationTime,
) -> Result<ControlDecision, NavigationRefusal> {
    if pose.validity.observed_at_ms > time.now_ms
        || time.now_ms > pose.validity.valid_until_ms
        || pose.clock_identity != time.clock_identity
        || trajectory.clock_identity != time.clock_identity
    {
        return Ok(ControlDecision::PoseStale {
            goal_identity: trajectory.goal_identity.clone(),
        });
    }
    if time.now_ms > trajectory.valid_until_ms {
        return Ok(ControlDecision::TrajectoryExpired {
            goal_identity: trajectory.goal_identity.clone(),
        });
    }
    if pose.frame != trajectory.frame
        || trajectory.segments.is_empty()
        || trajectory.segments.len() > NAVIGATION_MAXIMUM_SEGMENTS
    {
        return Err(NavigationRefusal::InvalidTrajectory);
    }
    let distance_squared = |segment: &TrajectorySegment| {
        let dx = i128::from(segment.target.x_mm) - i128::from(pose.x_mm);
        let dy = i128::from(segment.target.y_mm) - i128::from(pose.y_mm);
        dx * dx + dy * dy
    };
    let (nearest_index, nearest) = trajectory
        .segments
        .iter()
        .enumerate()
        .min_by_key(|(_, segment)| distance_squared(segment))
        .expect("nonempty trajectory was validated");
    let tolerance = i128::from(trajectory.position_tolerance_mm);
    let segment = if distance_squared(nearest) <= tolerance * tolerance {
        trajectory.segments.get(nearest_index + 1).copied()
    } else {
        Some(*nearest)
    };
    let Some(segment) = segment else {
        return terminal_heading_control(pose, trajectory);
    };
    control_segment(pose, trajectory, segment)
}

fn terminal_heading_control(
    pose: &NavigationPose,
    trajectory: &NavigationTrajectory,
) -> Result<ControlDecision, NavigationRefusal> {
    let error = normalized_heading_error(
        trajectory.target_heading_microdegrees,
        pose.heading_microdegrees,
    );
    if error.abs() <= i64::from(trajectory.heading_tolerance_microdegrees) {
        return Ok(ControlDecision::Arrived {
            goal_identity: trajectory.goal_identity.clone(),
        });
    }
    let segment = trajectory.segments[0];
    let maximum = i64::from(segment.maximum_angular_step_microdegrees);
    motion(trajectory, segment, 0, error.clamp(-maximum, maximum))
}

fn control_segment(
    pose: &NavigationPose,
    trajectory: &NavigationTrajectory,
    segment: TrajectorySegment,
) -> Result<ControlDecision, NavigationRefusal> {
    let dx = i64::from(segment.target.x_mm) - i64::from(pose.x_mm);
    let dy = i64::from(segment.target.y_mm) - i64::from(pose.y_mm);
    let desired = if dx.abs() >= dy.abs() {
        if dx >= 0 {
            0
        } else {
            180_000_000
        }
    } else if dy >= 0 {
        90_000_000
    } else {
        -90_000_000
    };
    let error = normalized_heading_error(desired, pose.heading_microdegrees);
    let maximum = i64::from(segment.maximum_angular_step_microdegrees);
    let angular = error.clamp(-maximum, maximum);
    let linear = if angular == 0 {
        dx.abs()
            .saturating_add(dy.abs())
            .min(i64::from(segment.maximum_linear_step_mm))
    } else {
        0
    };
    motion(trajectory, segment, linear, angular)
}

fn motion(
    trajectory: &NavigationTrajectory,
    segment: TrajectorySegment,
    linear: i64,
    angular: i64,
) -> Result<ControlDecision, NavigationRefusal> {
    Ok(ControlDecision::Motion(BoundedMotionIntent {
        goal_identity: trajectory.goal_identity.clone(),
        linear_mm: i32::try_from(linear).map_err(|_| NavigationRefusal::InvalidTrajectory)?,
        angular_microdegrees: i32::try_from(angular)
            .map_err(|_| NavigationRefusal::InvalidTrajectory)?,
        interval_ms: segment.interval_ms,
        ttl_ms: segment.interval_ms,
    }))
}

fn normalized_heading_error(target: i32, current: i32) -> i64 {
    const FULL: i64 = 360_000_000;
    const HALF: i64 = 180_000_000;
    (i64::from(target) - i64::from(current) + HALF).rem_euclid(FULL) - HALF
}
