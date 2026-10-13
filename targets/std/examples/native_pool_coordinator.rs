//! Internal preparation entrance for the live pool proof. This builds ordinary
//! Plans; it does not execute providers or implement an alternative selector.
#[path = "native_pool_coordinator/batch.rs"]
mod batch;
#[path = "native_pool_coordinator/costs.rs"]
mod costs;
#[path = "native_pool_coordinator/dispatch.rs"]
mod dispatch;
#[path = "native_pool_coordinator/evaluation.rs"]
mod evaluation;
#[path = "native_pool_coordinator/freshness.rs"]
mod freshness;
#[path = "native_pool_coordinator/identity.rs"]
mod identity;
#[path = "native_pool_coordinator/model_source.rs"]
mod model_source;
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
        Some("check-sample") => samples::run(false),
        Some("check-current-sample") => samples::run(true),
        Some("prepare-native-costs") => costs::run(false),
        Some("run-native-costs") => costs::run(true),
        Some("run-native-batch-costs") => batch::run(),
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
