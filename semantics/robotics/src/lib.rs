#![no_std]

extern crate alloc;

#[allow(dead_code, clippy::clone_on_copy)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));

    impl Copy for CliffSignalObserved {}
    impl Copy for CliffSignal {}
}

pub use generated::{
    AccelerationObservation, BatteryObservation, BeaconKind, BeaconKindCode, BeaconObservation,
    ButtonSetObservation, ChargingObservation, ChargingState, ChargingStateCode, CliffObservation,
    CliffSignal, CliffSignalObserved, ContactObservation, NavigationBoundedMotionIntent,
    NavigationControlDecision, NavigationControlDecisionArrived, NavigationControlDecisionHold,
    NavigationControlDecisionMotion, NavigationControlDecisionPoseStale,
    NavigationControlDecisionTrajectoryExpired, NavigationDecisionIdentity, NavigationGoal,
    NavigationGoalTarget, NavigationGoalTargetReach, NavigationIdentity64,
    NavigationPlanningInputIdentity, NavigationPose, NavigationRefusal, NavigationRoute,
    NavigationRouteDecision, NavigationRouteDecisionGoalInvalid, NavigationRouteDecisionHold,
    NavigationRouteDecisionNoPath, NavigationRouteDecisionObstacleDataUnavailable,
    NavigationRouteDecisionPoseStale, NavigationRouteDecisionRoute, NavigationRouteIdentity,
    NavigationTime, NavigationTrajectory, NavigationTrajectorySegment,
    NavigationTrajectorySegments, NavigationTraversability4x4, NavigationTraversabilityCell,
    NavigationTraversabilityCells, NavigationValidity, NavigationWaypoint, NavigationWaypoints,
    OdometryObservation, OrientationObservation, ProximityObservation, RangeObservation,
    RoboticsSimulationAvailability, WheelDropObservation,
};

macro_rules! navigation_text {
    ($type:ty) => {
        impl core::ops::Deref for $type {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.get().as_str()
            }
        }

        impl core::fmt::Display for $type {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str(self.get())
            }
        }
    };
}

navigation_text!(NavigationIdentity64);
navigation_text!(NavigationRouteIdentity);
navigation_text!(NavigationPlanningInputIdentity);

impl core::ops::Deref for NavigationTraversabilityCells {
    type Target = [NavigationTraversabilityCell; 16];

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl NavigationWaypoints {
    pub fn len(&self) -> usize {
        self.get().len()
    }

    pub fn is_empty(&self) -> bool {
        self.get().is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &NavigationWaypoint> {
        self.get().iter()
    }
}

impl NavigationTrajectorySegments {
    pub fn len(&self) -> usize {
        self.get().len()
    }

    pub fn is_empty(&self) -> bool {
        self.get().is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &NavigationTrajectorySegment> {
        self.get().iter()
    }

    pub fn at(&self, index: usize) -> Option<&NavigationTrajectorySegment> {
        self.get().iter().nth(index)
    }
}

impl core::ops::Index<usize> for NavigationTrajectorySegments {
    type Output = NavigationTrajectorySegment;

    fn index(&self, index: usize) -> &Self::Output {
        self.at(index).expect("navigation trajectory segment index")
    }
}

mod hazard_info;
mod info;
mod input_info;
mod structured;
mod structured_value;
mod structured_value_support;

pub use hazard_info::*;
pub use info::*;
pub use input_info::*;
pub use structured::*;
pub use structured_value::*;
