//! Finite implementation registry installed by the ordinary browser Host.

mod factory;
mod inventory;
mod limits;
mod linguistics;
mod logic;
mod math;
mod morse;
mod morse_composition;
mod operation;
mod presentation;
mod text;
mod values;

pub(crate) use factory::{
    advertisement, backs, catalogs, factory, local_bases, BrowserManifestation,
};
pub(crate) use inventory::inventory;
pub(crate) use limits::{
    envelope_limits, BROWSER_HOST_OPERATIONS_PER_GEAR, BROWSER_HOST_OPERATION_BINDINGS,
    BROWSER_PENDING_REQUESTS, BROWSER_PORTS_PER_GEAR, BROWSER_QUEUE_SLOTS, BROWSER_ROUTE_SLOTS,
    BROWSER_ROUTE_TARGETS, BROWSER_SIGN_ITEMS, BROWSER_TOTAL_VALUE_BYTES, BROWSER_VALUE_ITEMS,
    MAXIMUM_BROWSER_CORDS, MAXIMUM_BROWSER_GEARS, MAXIMUM_BROWSER_VALUE_BYTES,
};
pub(crate) use operation::BrowserOperation;
