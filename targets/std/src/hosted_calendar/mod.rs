//! Hosted calendar provider mechanisms below the portable calendar seam.

mod google_protocol;
mod google_response;
mod https_transport;
mod plan_contract;
mod semantic_service;

#[cfg(test)]
mod tests;

pub use google_protocol::*;
pub use google_response::{GoogleEventPage, GoogleFreeBusyPage, GoogleWriteReceipt};
pub use https_transport::GoogleHttpsTransport;
pub use plan_contract::{
    google_calendar_authority_grant, google_calendar_offers, google_calendar_resource_offer,
    CalendarHostedOperation, GOOGLE_CALENDAR_RESOURCE_CLASS,
};
pub use semantic_service::{GoogleCalendarService, HostedCalendarAdapter};
