//! Validated finite deterministic navigation reference realization.

use alloc::{format, vec, vec::Vec};

use conduit_robotics::NavigationRouteDecisionRoute;
pub use conduit_robotics::{
    NavigationBoundedMotionIntent as BoundedMotionIntent,
    NavigationControlDecision as ControlDecision, NavigationGoal,
    NavigationGoalTarget as GoalTarget, NavigationPose, NavigationRefusal, NavigationRoute,
    NavigationRouteDecision as RouteDecision, NavigationTime, NavigationTrajectory,
    NavigationTrajectorySegment as TrajectorySegment,
    NavigationTraversability4x4 as Traversability4x4,
    NavigationTraversabilityCell as TraversabilityCell, NavigationValidity as Validity,
    NavigationWaypoint as Waypoint,
};

use crate::{
    NAVIGATION_MAXIMUM_PLANNING_INPUT_IDENTITY_BYTES, NAVIGATION_MAXIMUM_ROUTE_IDENTITY_BYTES,
    NAVIGATION_MAXIMUM_SEGMENTS, NAVIGATION_MAXIMUM_SOURCE_IDENTITY_BYTES,
    NAVIGATION_MAXIMUM_WAYPOINTS,
};

pub fn validate_identity(value: &str) -> Result<(), NavigationRefusal> {
    validate_identity_with_bound(value, NAVIGATION_MAXIMUM_SOURCE_IDENTITY_BYTES)
}

fn validate_identity_with_bound(
    value: &str,
    maximum_bytes: usize,
) -> Result<(), NavigationRefusal> {
    if value.is_empty() {
        Err(NavigationRefusal::EmptyIdentity)
    } else if value.len() > maximum_bytes {
        Err(NavigationRefusal::IdentityTooLong)
    } else {
        Ok(())
    }
}

fn current(validity: Validity, now_ms: u64) -> bool {
    validity.observed_at_ms <= now_ms && now_ms <= validity.valid_until_ms
}

pub fn route_grid4(
    pose: &NavigationPose,
    goal: &NavigationGoal,
    grid: &Traversability4x4,
    time: &NavigationTime,
) -> Result<RouteDecision, NavigationRefusal> {
    for identity in [
        &pose.source_identity,
        &pose.clock_identity,
        &pose.frame,
        &goal.identity,
        &goal.clock_identity,
        &grid.source_identity,
        &grid.clock_identity,
        &grid.frame,
        &time.clock_identity,
    ] {
        validate_identity(identity)?;
    }
    if let GoalTarget::Reach(reach) = &goal.target {
        validate_identity(reach.frame())?;
    }
    if pose.validity.observed_at_ms > pose.validity.valid_until_ms
        || grid.validity.observed_at_ms > grid.validity.valid_until_ms
    {
        return Err(NavigationRefusal::InvalidValidity);
    }
    if !current(pose.validity, time.now_ms)
        || pose.clock_identity != goal.clock_identity
        || pose.clock_identity != time.clock_identity
    {
        return RouteDecision::pose_stale(goal.identity.clone())
            .map_err(|_| NavigationRefusal::InvalidRoute);
    }
    if time.now_ms > goal.valid_until_ms {
        return RouteDecision::goal_invalid(goal.identity.clone())
            .map_err(|_| NavigationRefusal::InvalidRoute);
    }
    if matches!(goal.target, GoalTarget::Hold) {
        return RouteDecision::hold(goal.identity.clone())
            .map_err(|_| NavigationRefusal::InvalidRoute);
    }
    if !current(grid.validity, time.now_ms)
        || grid.clock_identity != goal.clock_identity
        || grid.clock_identity != time.clock_identity
    {
        return RouteDecision::obstacle_data_unavailable(goal.identity.clone())
            .map_err(|_| NavigationRefusal::InvalidRoute);
    }
    if grid.cell_width_mm == 0
        || grid.cell_height_mm == 0
        || i32::try_from(grid.cell_width_mm).is_err()
        || i32::try_from(grid.cell_height_mm).is_err()
        || i64::from(grid.origin_x_mm) + 4 * i64::from(grid.cell_width_mm) > i64::from(i32::MAX)
        || i64::from(grid.origin_y_mm) + 4 * i64::from(grid.cell_height_mm) > i64::from(i32::MAX)
    {
        return Err(NavigationRefusal::InvalidGrid);
    }
    let GoalTarget::Reach(reach) = &goal.target else {
        unreachable!()
    };
    if pose.frame != *reach.frame()
        || grid.frame != *reach.frame()
        || grid.cell_width_mm == 0
        || grid.cell_height_mm == 0
    {
        return RouteDecision::goal_invalid(goal.identity.clone())
            .map_err(|_| NavigationRefusal::InvalidRoute);
    }
    let Some(start) = cell_for(grid, pose.x_mm, pose.y_mm) else {
        return RouteDecision::goal_invalid(goal.identity.clone())
            .map_err(|_| NavigationRefusal::InvalidRoute);
    };
    let Some(end) = cell_for(grid, *reach.x_mm(), *reach.y_mm()) else {
        return RouteDecision::goal_invalid(goal.identity.clone())
            .map_err(|_| NavigationRefusal::InvalidRoute);
    };
    let candidates = [
        [start, (end.0, start.1), end],
        [start, (start.0, end.1), end],
    ];
    let mut saw_unknown = false;
    for candidate in candidates {
        match clear_leg(grid, candidate[0], candidate[1]).and(clear_leg(
            grid,
            candidate[1],
            candidate[2],
        )) {
            Leg::Clear => {
                let mut waypoints = vec![Waypoint {
                    x_mm: pose.x_mm,
                    y_mm: pose.y_mm,
                }];
                let corner = cell_center(grid, candidate[1]);
                if candidate[1] != start && candidate[1] != end {
                    waypoints.push(corner);
                }
                // Keep the terminal waypoint even when the position is already
                // satisfied: the controller may still need to satisfy the
                // independently bounded terminal heading.
                waypoints.push(Waypoint {
                    x_mm: *reach.x_mm(),
                    y_mm: *reach.y_mm(),
                });
                if waypoints.is_empty() || waypoints.len() > NAVIGATION_MAXIMUM_WAYPOINTS {
                    return Err(NavigationRefusal::InvalidRoute);
                }
                let identity = conduit_robotics::NavigationRouteIdentity::new(format!(
                    "route/{}",
                    goal.identity
                ))
                .map_err(|_| NavigationRefusal::InvalidRoute)?;
                let planner_identity = conduit_robotics::NavigationIdentity64::new(
                    "navigation/deterministic-grid4@1".into(),
                )
                .map_err(|_| NavigationRefusal::InvalidRoute)?;
                let planning_input_identity =
                    conduit_robotics::NavigationPlanningInputIdentity::new(format!(
                        "goal={};pose={}#{};grid={}#{}",
                        goal.identity,
                        pose.source_identity,
                        pose.sample_sequence,
                        grid.source_identity,
                        grid.sample_sequence,
                    ))
                    .map_err(|_| NavigationRefusal::InvalidRoute)?;
                let bounded = conduit_plot::rust_binding::BoundedSequence::try_from_iter(waypoints)
                    .map_err(|_| NavigationRefusal::InvalidRoute)?;
                let waypoints = conduit_robotics::NavigationWaypoints::new(bounded)
                    .map_err(|_| NavigationRefusal::InvalidRoute)?;
                return RouteDecision::route(
                    reach.frame().clone(),
                    goal.identity.clone(),
                    *reach.heading_tolerance_microdegrees(),
                    identity,
                    planner_identity,
                    planning_input_identity,
                    *reach.position_tolerance_mm(),
                    *reach.heading_microdegrees(),
                    waypoints,
                )
                .map_err(|_| NavigationRefusal::InvalidRoute);
            }
            Leg::Unknown => saw_unknown = true,
            Leg::Blocked => {}
        }
    }
    Ok(if saw_unknown {
        RouteDecision::obstacle_data_unavailable(goal.identity.clone())
            .map_err(|_| NavigationRefusal::InvalidRoute)?
    } else {
        RouteDecision::no_path(goal.identity.clone())
            .map_err(|_| NavigationRefusal::InvalidRoute)?
    })
}

#[derive(Clone, Copy)]
enum Leg {
    Clear,
    Blocked,
    Unknown,
}
impl Leg {
    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::Blocked, _) | (_, Self::Blocked) => Self::Blocked,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::Clear,
        }
    }
}

fn cell_for(grid: &Traversability4x4, x: i32, y: i32) -> Option<(usize, usize)> {
    let dx = i64::from(x) - i64::from(grid.origin_x_mm);
    let dy = i64::from(y) - i64::from(grid.origin_y_mm);
    if dx < 0 || dy < 0 {
        return None;
    }
    let column = usize::try_from(dx / i64::from(grid.cell_width_mm)).ok()?;
    let row = usize::try_from(dy / i64::from(grid.cell_height_mm)).ok()?;
    (column < 4 && row < 4).then_some((column, row))
}

fn cell_center(grid: &Traversability4x4, cell: (usize, usize)) -> Waypoint {
    let center = |origin: i32, index: usize, extent: u32| {
        i64::from(origin) + index as i64 * i64::from(extent) + i64::from(extent / 2)
    };
    Waypoint {
        x_mm: i32::try_from(center(grid.origin_x_mm, cell.0, grid.cell_width_mm))
            .expect("validated grid x center fits i32"),
        y_mm: i32::try_from(center(grid.origin_y_mm, cell.1, grid.cell_height_mm))
            .expect("validated grid y center fits i32"),
    }
}

fn clear_leg(grid: &Traversability4x4, from: (usize, usize), to: (usize, usize)) -> Leg {
    if from.0 != to.0 && from.1 != to.1 {
        return Leg::Blocked;
    }
    let mut result = Leg::Clear;
    let (mut x, mut y) = (from.0 as isize, from.1 as isize);
    let (tx, ty) = (to.0 as isize, to.1 as isize);
    loop {
        match grid.cells[y as usize * 4 + x as usize] {
            TraversabilityCell::Blocked => return Leg::Blocked,
            TraversabilityCell::Unknown => result = Leg::Unknown,
            TraversabilityCell::Free => {}
        }
        if (x, y) == (tx, ty) {
            break;
        }
        x += (tx - x).signum();
        y += (ty - y).signum();
    }
    result
}

trait RouteView {
    fn identity(&self) -> &conduit_robotics::NavigationRouteIdentity;
    fn goal_identity(&self) -> &conduit_robotics::NavigationIdentity64;
    fn planner_identity(&self) -> &conduit_robotics::NavigationIdentity64;
    fn planning_input_identity(&self) -> &conduit_robotics::NavigationPlanningInputIdentity;
    fn frame(&self) -> &conduit_robotics::NavigationIdentity64;
    fn target_heading_microdegrees(&self) -> i32;
    fn position_tolerance_mm(&self) -> u32;
    fn heading_tolerance_microdegrees(&self) -> u32;
    fn waypoints(&self) -> &conduit_robotics::NavigationWaypoints;
}

macro_rules! route_view {
    ($type:ty) => {
        impl RouteView for $type {
            fn identity(&self) -> &conduit_robotics::NavigationRouteIdentity {
                self.identity()
            }
            fn goal_identity(&self) -> &conduit_robotics::NavigationIdentity64 {
                self.goal_identity()
            }
            fn planner_identity(&self) -> &conduit_robotics::NavigationIdentity64 {
                self.planner_identity()
            }
            fn planning_input_identity(
                &self,
            ) -> &conduit_robotics::NavigationPlanningInputIdentity {
                self.planning_input_identity()
            }
            fn frame(&self) -> &conduit_robotics::NavigationIdentity64 {
                self.frame()
            }
            fn target_heading_microdegrees(&self) -> i32 {
                *self.target_heading_microdegrees()
            }
            fn position_tolerance_mm(&self) -> u32 {
                *self.position_tolerance_mm()
            }
            fn heading_tolerance_microdegrees(&self) -> u32 {
                *self.heading_tolerance_microdegrees()
            }
            fn waypoints(&self) -> &conduit_robotics::NavigationWaypoints {
                self.waypoints()
            }
        }
    };
}
route_view!(NavigationRoute);
route_view!(NavigationRouteDecisionRoute);

pub fn time_parameterize(
    route: &NavigationRouteDecisionRoute,
    time: &NavigationTime,
    interval_ms: u32,
    maximum_linear_step_mm: u32,
    maximum_angular_step_microdegrees: u32,
) -> Result<NavigationTrajectory, NavigationRefusal> {
    time_parameterize_view(
        route,
        time,
        interval_ms,
        maximum_linear_step_mm,
        maximum_angular_step_microdegrees,
    )
}

pub fn time_parameterize_route(
    route: &NavigationRoute,
    time: &NavigationTime,
    interval_ms: u32,
    maximum_linear_step_mm: u32,
    maximum_angular_step_microdegrees: u32,
) -> Result<NavigationTrajectory, NavigationRefusal> {
    time_parameterize_view(
        route,
        time,
        interval_ms,
        maximum_linear_step_mm,
        maximum_angular_step_microdegrees,
    )
}

fn time_parameterize_view(
    route: &impl RouteView,
    time: &NavigationTime,
    interval_ms: u32,
    maximum_linear_step_mm: u32,
    maximum_angular_step_microdegrees: u32,
) -> Result<NavigationTrajectory, NavigationRefusal> {
    validate_identity_with_bound(route.identity(), NAVIGATION_MAXIMUM_ROUTE_IDENTITY_BYTES)?;
    validate_identity(route.goal_identity())?;
    validate_identity(route.planner_identity())?;
    validate_identity_with_bound(
        route.planning_input_identity(),
        NAVIGATION_MAXIMUM_PLANNING_INPUT_IDENTITY_BYTES,
    )?;
    validate_identity(route.frame())?;
    validate_identity(&time.clock_identity)?;
    if route.waypoints().len() < 2
        || route.waypoints().len() > NAVIGATION_MAXIMUM_WAYPOINTS
        || interval_ms == 0
        || maximum_linear_step_mm == 0
        || maximum_angular_step_microdegrees == 0
    {
        return Err(NavigationRefusal::InvalidRoute);
    }
    let segments: Vec<_> = route
        .waypoints()
        .iter()
        .skip(1)
        .map(|target| TrajectorySegment {
            target: *target,
            interval_ms,
            maximum_linear_step_mm,
            maximum_angular_step_microdegrees,
        })
        .collect();
    if segments.is_empty() || segments.len() > NAVIGATION_MAXIMUM_SEGMENTS {
        return Err(NavigationRefusal::InvalidTrajectory);
    }
    let segments = conduit_plot::rust_binding::BoundedSequence::try_from_iter(segments)
        .map_err(|_| NavigationRefusal::InvalidTrajectory)?;
    let segments = conduit_robotics::NavigationTrajectorySegments::new(segments)
        .map_err(|_| NavigationRefusal::InvalidTrajectory)?;
    NavigationTrajectory::new(
        time.clock_identity.clone(),
        route.frame().clone(),
        route.goal_identity().clone(),
        route.heading_tolerance_microdegrees(),
        route.position_tolerance_mm(),
        route.identity().clone(),
        segments,
        route.target_heading_microdegrees(),
        time.now_ms
            .saturating_add(u64::from(interval_ms) * (route.waypoints().len() - 1) as u64),
    )
    .map_err(|_| NavigationRefusal::InvalidTrajectory)
}
