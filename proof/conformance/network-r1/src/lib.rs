#![no_std]

extern crate alloc;

mod r1_route;
pub use r1_route::*;
mod r1_wifi_bootstrap;
pub use r1_wifi_bootstrap::*;

pub const R1_WIFI_STATION_POOL_ID: &str = "r1/pico-wifi-station-0";
