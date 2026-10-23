//! CPU integration of the production Root broker with an explicit service fixture.
//! No protected entry, guest heap, scheduler or hostile-code confinement claim.
extern crate alloc;
#[path = "../src/protected_application.rs"]
mod protected_application;
