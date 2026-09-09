//! Validated finite deterministic navigation reference realization.

use alloc::{format, string::String, vec, vec::Vec};

use crate::{
    NAVIGATION_MAXIMUM_IDENTITY_BYTES, NAVIGATION_MAXIMUM_SEGMENTS, NAVIGATION_MAXIMUM_WAYPOINTS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Validity {
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationTime {
    pub clock_identity: String,
    pub now_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationPose {
    pub source_identity: String,
    pub sample_sequence: u64,
    pub clock_identity: String,
    pub frame: String,
    pub x_mm: i32,
    pub y_mm: i32,
    pub heading_microdegrees: i32,
    pub validity: Validity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GoalTarget {
    Hold,
    Reach {
        frame: String,
        x_mm: i32,
        y_mm: i32,
        heading_microdegrees: i32,
        position_tolerance_mm: u32,
        heading_tolerance_microdegrees: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationGoal {
    pub identity: String,
    pub clock_identity: String,
    pub valid_until_ms: u64,
    pub target: GoalTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraversabilityCell {
    Free,
    Blocked,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Traversability4x4 {
    pub source_identity: String,
    pub sample_sequence: u64,
    pub clock_identity: String,
    pub frame: String,
    pub origin_x_mm: i32,
    pub origin_y_mm: i32,
    pub cell_width_mm: u32,
    pub cell_height_mm: u32,
    pub validity: Validity,
    pub cells: [TraversabilityCell; 16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Waypoint {
    pub x_mm: i32,
    pub y_mm: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationRoute {
    pub identity: String,
    pub goal_identity: String,
    pub planner_identity: String,
    pub planning_input_identity: String,
    pub frame: String,
    pub target_heading_microdegrees: i32,
    pub position_tolerance_mm: u32,
    pub heading_tolerance_microdegrees: u32,
    pub waypoints: Vec<Waypoint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteDecision {
    Route(NavigationRoute),
    Hold { goal_identity: String },
    NoPath { goal_identity: String },
    GoalInvalid { goal_identity: String },
    ObstacleDataUnavailable { goal_identity: String },
    PoseStale { goal_identity: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrajectorySegment {
    pub target: Waypoint,
    pub interval_ms: u32,
    pub maximum_linear_step_mm: u32,
    pub maximum_angular_step_microdegrees: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationTrajectory {
    pub route_identity: String,
    pub goal_identity: String,
    pub clock_identity: String,
    pub valid_until_ms: u64,
    pub frame: String,
    pub target_heading_microdegrees: i32,
    pub position_tolerance_mm: u32,
    pub heading_tolerance_microdegrees: u32,
    pub segments: Vec<TrajectorySegment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedMotionIntent {
    pub goal_identity: String,
    pub linear_mm: i32,
    pub angular_microdegrees: i32,
    pub interval_ms: u32,
    pub ttl_ms: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlDecision {
    Motion(BoundedMotionIntent),
    Arrived { goal_identity: String },
    Hold { goal_identity: String },
    PoseStale { goal_identity: String },
    TrajectoryExpired { goal_identity: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationRefusal {
    EmptyIdentity,
    IdentityTooLong,
    InvalidValidity,
    InvalidGrid,
    InvalidRoute,
    InvalidTrajectory,
}

pub fn validate_identity(value: &str) -> Result<(), NavigationRefusal> {
    if value.is_empty() {
        Err(NavigationRefusal::EmptyIdentity)
    } else if value.len() > NAVIGATION_MAXIMUM_IDENTITY_BYTES {
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
        pose.source_identity.as_str(),
        pose.clock_identity.as_str(),
        pose.frame.as_str(),
        goal.identity.as_str(),
        goal.clock_identity.as_str(),
        grid.source_identity.as_str(),
        grid.clock_identity.as_str(),
        grid.frame.as_str(),
        time.clock_identity.as_str(),
    ] {
        validate_identity(identity)?;
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
        return Ok(RouteDecision::PoseStale {
            goal_identity: goal.identity.clone(),
        });
    }
    if time.now_ms > goal.valid_until_ms {
        return Ok(RouteDecision::GoalInvalid {
            goal_identity: goal.identity.clone(),
        });
    }
    if matches!(goal.target, GoalTarget::Hold) {
        return Ok(RouteDecision::Hold {
            goal_identity: goal.identity.clone(),
        });
    }
    if !current(grid.validity, time.now_ms)
        || grid.clock_identity != goal.clock_identity
        || grid.clock_identity != time.clock_identity
    {
        return Ok(RouteDecision::ObstacleDataUnavailable {
            goal_identity: goal.identity.clone(),
        });
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
    let GoalTarget::Reach {
        frame,
        x_mm,
        y_mm,
        heading_microdegrees,
        position_tolerance_mm,
        heading_tolerance_microdegrees,
    } = &goal.target
    else {
        unreachable!()
    };
    if pose.frame != *frame
        || grid.frame != *frame
        || grid.cell_width_mm == 0
        || grid.cell_height_mm == 0
    {
        return Ok(RouteDecision::GoalInvalid {
            goal_identity: goal.identity.clone(),
        });
    }
    let Some(start) = cell_for(grid, pose.x_mm, pose.y_mm) else {
        return Ok(RouteDecision::GoalInvalid {
            goal_identity: goal.identity.clone(),
        });
    };
    let Some(end) = cell_for(grid, *x_mm, *y_mm) else {
        return Ok(RouteDecision::GoalInvalid {
            goal_identity: goal.identity.clone(),
        });
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
                    x_mm: *x_mm,
                    y_mm: *y_mm,
                });
                if waypoints.is_empty() || waypoints.len() > NAVIGATION_MAXIMUM_WAYPOINTS {
                    return Err(NavigationRefusal::InvalidRoute);
                }
                return Ok(RouteDecision::Route(NavigationRoute {
                    identity: format!("route/{}", goal.identity),
                    goal_identity: goal.identity.clone(),
                    planner_identity: "navigation/deterministic-grid4@1".into(),
                    planning_input_identity: format!(
                        "goal={};pose={}#{};grid={}#{}",
                        goal.identity,
                        pose.source_identity,
                        pose.sample_sequence,
                        grid.source_identity,
                        grid.sample_sequence,
                    ),
                    frame: frame.clone(),
                    target_heading_microdegrees: *heading_microdegrees,
                    position_tolerance_mm: *position_tolerance_mm,
                    heading_tolerance_microdegrees: *heading_tolerance_microdegrees,
                    waypoints,
                }));
            }
            Leg::Unknown => saw_unknown = true,
            Leg::Blocked => {}
        }
    }
    Ok(if saw_unknown {
        RouteDecision::ObstacleDataUnavailable {
            goal_identity: goal.identity.clone(),
        }
    } else {
        RouteDecision::NoPath {
            goal_identity: goal.identity.clone(),
        }
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

pub fn time_parameterize(
    route: &NavigationRoute,
    time: &NavigationTime,
    interval_ms: u32,
    maximum_linear_step_mm: u32,
    maximum_angular_step_microdegrees: u32,
) -> Result<NavigationTrajectory, NavigationRefusal> {
    validate_identity(&route.identity)?;
    validate_identity(&route.goal_identity)?;
    validate_identity(&time.clock_identity)?;
    if route.waypoints.len() < 2
        || route.waypoints.len() > NAVIGATION_MAXIMUM_WAYPOINTS
        || interval_ms == 0
        || maximum_linear_step_mm == 0
        || maximum_angular_step_microdegrees == 0
    {
        return Err(NavigationRefusal::InvalidRoute);
    }
    let segments: Vec<_> = route
        .waypoints
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
    Ok(NavigationTrajectory {
        route_identity: route.identity.clone(),
        goal_identity: route.goal_identity.clone(),
        clock_identity: time.clock_identity.clone(),
        valid_until_ms: time
            .now_ms
            .saturating_add(u64::from(interval_ms) * segments.len() as u64),
        frame: route.frame.clone(),
        target_heading_microdegrees: route.target_heading_microdegrees,
        position_tolerance_mm: route.position_tolerance_mm,
        heading_tolerance_microdegrees: route.heading_tolerance_microdegrees,
        segments,
    })
}
