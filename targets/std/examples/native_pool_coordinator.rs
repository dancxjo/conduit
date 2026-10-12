//! Internal preparation entrance for the live pool proof. This builds ordinary
//! Plans; it does not execute providers or implement an alternative selector.
#[path = "native_pool_coordinator/planning.rs"]
mod planning;
#[path = "native_pool_coordinator/samples.rs"]
mod samples;
fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        None | Some("plan") => planning::run(),
        Some("check-sample") => samples::run(),
        _ => Err("unknown preparation command".into()),
    };
    if let Err(error) = result {
        eprintln!("native pool planning refused: {error}");
        std::process::exit(1);
    }
}
