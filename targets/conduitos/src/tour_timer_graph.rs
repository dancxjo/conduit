//! Root retains only bounded preparation data and its encoder, never the scheduler.
pub(crate) const NODES: usize = 3;
pub(crate) const CORDS: usize = 2;
pub(crate) const PORTS: usize = 1;
#[path = "tour_timer_runtime/preparation.rs"]
mod preparation;
#[path = "tour_timer_runtime/wire.rs"]
mod wire;
pub(crate) use preparation::{PreparedTimerGraph, PreparedTimerRoute};
