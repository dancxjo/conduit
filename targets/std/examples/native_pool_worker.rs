//! Internal worker for the opt-in multi-Host proof. The coordinator owns
//! process deadlines, provider lifecycle, planning, and common pool selection.
#[path = "native_pool_worker/worker.rs"]
mod worker;
fn main() {
    if let Err(error) = worker::run() {
        eprintln!("native pool worker refused: {error}");
        std::process::exit(1);
    }
}
