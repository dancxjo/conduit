//! Internal preparation entrance for the live pool proof. This builds ordinary
//! Plans; it does not execute providers or implement an alternative selector.
#[path = "native_pool_coordinator/freshness.rs"]
mod freshness;
#[path = "native_pool_coordinator/identity.rs"]
mod identity;
#[path = "native_pool_coordinator/overlap.rs"]
mod overlap;
#[path = "native_pool_coordinator/planning.rs"]
mod planning;
#[path = "native_pool_coordinator/reference.rs"]
mod reference;
#[path = "native_pool_coordinator/samples.rs"]
mod samples;
#[path = "native_pool_coordinator/sessions.rs"]
mod sessions;
fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        None | Some("plan") => planning::run(),
        Some("check-sample") => samples::run(),
        Some("check-overlap") => overlap::run(),
        Some("run-reference") => reference::run(),
        Some("run-sessions") => sessions::run(),
        Some("initialize-reference") => identity::run(),
        _ => Err("unknown preparation command".into()),
    };
    if let Err(error) = result {
        eprintln!("native pool planning refused: {error}");
        std::process::exit(1);
    }
}
