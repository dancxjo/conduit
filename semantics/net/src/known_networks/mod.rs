//! Private bounded known-network data and nonauthorizing local selection.
//!
//! These values establish neither native possession, a Line, nor Body membership.
//! Root must authenticate/freshness-check peer observations, preserve Host/Boot/Body
//! identity across attachment changes, and retire current affine owners and their
//! mandatory evidence before executing a selected replacement. Scan observations
//! are hints, never credentials or membership proof. Policy state must remain
//! owned across polling/reconnect; rebuilding it to evade attempt bounds is invalid.
//! A prolonged outage may request a fresh bounded recovery round after cooldown
//! and a fresh known-network scan. Only the integrating Root may confirm actual
//! AP restoration plus affine owner/evidence retirement and complete that round
//! renewal. A scan, elapsed timer or missing outcome cannot silently renew it.
mod policy;
mod profiles;
pub use policy::*;
pub use profiles::*;
#[cfg(test)]
mod tests;
