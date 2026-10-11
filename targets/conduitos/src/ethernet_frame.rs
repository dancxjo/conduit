//! New bounded raw Ethernet prerequisite; not yet installed by native Root.
//! The queues connect checked frame Calls to the existing smoltcp Device API.
//! No USB discovery, descriptor policy, separate stack, or ambient authority.
#![cfg_attr(not(test), allow(dead_code))] // Ordinary Root installation is a separate integration step.
pub mod contract;
#[cfg(feature = "native-owner-network")]
pub mod device;
pub mod factory;
#[cfg(feature = "native-owner-network")]
pub mod installation;
#[cfg(feature = "native-owner-network")]
pub mod network_session;
pub mod offer;
pub(crate) mod owner;
pub mod queues;
