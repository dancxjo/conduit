//! Finite implementation registry installed by the ordinary browser Host.

mod button_indicator;
mod delay;
mod factory;
mod input;
mod inventory;
mod layout;
mod limits;
mod linguistics;
mod logic;
mod math;
mod morse;
mod morse_composition;
mod operation;
mod presentation;
mod state_time;
mod text;
mod values;

pub(crate) use factory::{
    advertisement, backs, catalogs, factory, local_bases, membership_advertisement,
    selected_human_machinery, BrowserManifestation,
};
pub(crate) use input::{BUTTON_EVENT_OPERATION, KEY_EVENT_OPERATION};
pub(crate) use inventory::inventory;
pub(crate) use limits::{
    envelope_limits, BROWSER_HOST_OPERATIONS_PER_GEAR, BROWSER_HOST_OPERATION_BINDINGS,
    BROWSER_PENDING_REQUESTS, BROWSER_PORTS_PER_GEAR, BROWSER_QUEUE_SLOTS, BROWSER_ROUTE_SLOTS,
    BROWSER_ROUTE_TARGETS, BROWSER_SIGN_ITEMS, BROWSER_TOTAL_VALUE_BYTES, BROWSER_VALUE_ITEMS,
    MAXIMUM_BROWSER_CORDS, MAXIMUM_BROWSER_GEARS, MAXIMUM_BROWSER_VALUE_BYTES,
};
pub(crate) use operation::BrowserOperation;
pub(crate) use state_time::BROWSER_TIMER_MAXIMUM_MILLIS;
