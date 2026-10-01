#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    AccelerationObservation, BatteryObservation, BeaconKind, BeaconKindCode, ButtonSetObservation,
    ChargingState,
    ChargingStateCode, ContactObservation, OdometryObservation, ProximityObservation,
    RangeObservation, RoboticsSimulationAvailability, WheelDropObservation,
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
