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
    CliffSignal, CliffSignalObserved, ContactObservation, OdometryObservation,
    OrientationObservation, ProximityObservation, RangeObservation, RoboticsSimulationAvailability,
    WheelDropObservation,
};

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
