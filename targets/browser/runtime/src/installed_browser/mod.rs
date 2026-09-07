//! Finite implementation registry installed by the ordinary browser Host.

mod button_indicator;
mod delay;
mod factory;
mod final_normalized_pattern;
pub(crate) mod historical;
mod input;
mod inventory;
pub(crate) mod json;
mod layout;
mod limits;
mod linguistics;
mod logic;
mod math;
pub(crate) mod measurement_plot;
pub(crate) mod measurement_summary;
pub(crate) mod measurement_window;
mod membership_offer;
mod morse;
mod morse_composition;
mod normalized_quantity;
mod operation;
mod phase_synchronization;
mod pointer;
pub(crate) mod pointer_selector;
mod presentation;
mod pulse_observation;
mod pulse_presentation;
mod quantity;
mod quantity_output;
pub(crate) mod record_delivery;
pub(crate) mod record_queue;
mod record_temporal;
mod record_transcript;
pub(crate) mod replay_control;
pub(crate) mod replay_source;
pub(crate) mod resource;
mod rhythm_presentation;
mod rhythm_state;
mod state_time;
pub(crate) mod structured_selector;
pub(crate) mod template_storage;
mod text;
mod tick;
pub(crate) mod timing;
pub(crate) mod typed_record;
mod values;

#[cfg(test)]
mod secret_knock_trigger_plan;

pub(crate) use factory::{
    advertisement, backs, catalogs, factory, local_bases, selected_human_machinery,
    BrowserManifestation,
};
pub(crate) use factory::{
    advertisement_for_presentation, catalogs_for_presentation, execution_capability_ids,
};
pub(crate) use input::{BUTTON_EVENT_OPERATION, KEY_EVENT_OPERATION};
pub(crate) use inventory::inventory;
pub(crate) use limits::{
    envelope_limits, BROWSER_HOST_OPERATIONS_PER_GEAR, BROWSER_HOST_OPERATION_BINDINGS,
    BROWSER_PENDING_REQUESTS, BROWSER_PORTS_PER_GEAR, BROWSER_QUEUE_SLOTS, BROWSER_ROUTE_SLOTS,
    BROWSER_ROUTE_TARGETS, BROWSER_SIGN_ITEMS, BROWSER_TOTAL_VALUE_BYTES, BROWSER_VALUE_ITEMS,
    MAXIMUM_BROWSER_CORDS, MAXIMUM_BROWSER_GEARS, MAXIMUM_BROWSER_VALUE_BYTES,
};
pub(crate) use membership_offer::advertisement as membership_advertisement;
pub(crate) use normalized_quantity::{
    transform as normalize_quantity, HOST_OPERATION as NORMALIZE_QUANTITY_OPERATION,
};
pub(crate) use operation::BrowserOperation;

fn record_delivery_refusal_detail(refusal: conduit_net::RecordDeliveryRefusal) -> u16 {
    use conduit_net::RecordDeliveryRefusal::*;
    match refusal {
        EmptyCorrelation => 1,
        CorrelationTooLong => 2,
        EmptyFrame => 3,
        FrameTooLarge => 4,
        InvalidTransition => 5,
        InvalidPartialProgress => 6,
        EmptyReceipt => 7,
        ReceiptTooLong => 8,
        OutputTooSmall => 9,
        MalformedWire => 10,
        ObservationIdentityMismatch => 11,
    }
}
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

#[cfg(test)]
pub(crate) mod test_replay_sink;

#[cfg(test)]
pub(crate) mod test_measurement_sink;

pub(crate) mod button_attempt;

pub(crate) mod catalogs;

pub(crate) mod normalized_presentation;
pub(crate) use catalogs::PresentationProfile;

pub(crate) mod pattern_comparison;

pub(crate) mod comparison_presentation;
