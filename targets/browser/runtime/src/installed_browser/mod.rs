//! Finite implementation registry installed by the ordinary browser Host.

mod button_indicator;
mod delay;
mod factory;
mod input;
mod inventory;
pub(crate) mod json;
mod layout;
mod limits;
mod linguistics;
mod logic;
mod math;
mod morse;
mod morse_composition;
mod normalized_quantity;
mod operation;
mod pointer;
pub(crate) mod pointer_selector;
mod presentation;
mod quantity;
mod quantity_output;
pub(crate) mod resource;
mod state_time;
mod text;
mod tick_presentation;
pub(crate) mod timing;
mod values;

#[cfg(test)]
mod secret_knock_trigger_plan;

pub(crate) use factory::{
    advertisement, backs, catalogs, factory, local_bases, membership_advertisement,
    selected_human_machinery, BrowserManifestation,
};
pub(crate) use factory::{advertisement_for_presentation, catalogs_for_presentation};
pub(crate) use input::{BUTTON_EVENT_OPERATION, KEY_EVENT_OPERATION};
pub(crate) use inventory::inventory;
pub(crate) use limits::{
    envelope_limits, BROWSER_HOST_OPERATIONS_PER_GEAR, BROWSER_HOST_OPERATION_BINDINGS,
    BROWSER_PENDING_REQUESTS, BROWSER_PORTS_PER_GEAR, BROWSER_QUEUE_SLOTS, BROWSER_ROUTE_SLOTS,
    BROWSER_ROUTE_TARGETS, BROWSER_SIGN_ITEMS, BROWSER_TOTAL_VALUE_BYTES, BROWSER_VALUE_ITEMS,
    MAXIMUM_BROWSER_CORDS, MAXIMUM_BROWSER_GEARS, MAXIMUM_BROWSER_VALUE_BYTES,
};
pub(crate) use normalized_quantity::{
    transform as normalize_quantity, HOST_OPERATION as NORMALIZE_QUANTITY_OPERATION,
};
pub(crate) use operation::BrowserOperation;
pub(crate) use pointer::HOST_OPERATION as POINTER_EVENT_OPERATION;
pub(crate) use quantity::{
    configuration as prepare_quantity_mapping, transform as transform_quantity,
    HOST_OPERATION as QUANTITY_HOST_OPERATION,
};
pub(crate) use quantity_output::{
    decode as decode_quantity_leaf, wrap as wrap_quantity,
    WRAP_OPERATION as QUANTITY_WRAP_OPERATION,
};
pub(crate) use state_time::BROWSER_TIMER_MAXIMUM_MILLIS;

#[cfg(test)]
pub(crate) mod test_json;

#[cfg(test)]
pub(crate) mod test_timing_sink;

pub(crate) mod button_attempt;

mod catalogs;

pub(crate) mod normalized_presentation;
pub(crate) use catalogs::PresentationProfile;

pub(crate) mod pattern_comparison;

pub(crate) mod comparison_presentation;
