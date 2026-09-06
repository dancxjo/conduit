//! Shared executable-tour runner extended across two exact browser fragments.

mod abi;
#[cfg(test)]
pub(super) use abi::{conduit_tour_multi_start_sink, conduit_tour_multi_start_source};
mod plan;
mod protocol;
mod session;

pub(super) fn has_session() -> bool {
    abi::has_session()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod button_tests;
