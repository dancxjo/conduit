//! Shared held-start/release transitions for Root-owned execution custody.
//! Callers independently check actual providers, storage and authenticated scope.
pub fn accept_start(
    phase: &mut &'static str,
    cancelled: bool,
    ready: bool,
) -> Result<(), &'static str> {
    if cancelled {
        return Err("worker-cancellation-requested");
    }
    if *phase != "ready-held" || !ready {
        return Err("worker-not-ready-held");
    }
    *phase = "started-held";
    Ok(())
}
pub fn release(phase: &mut &'static str, cancelled: bool) -> Result<(), &'static str> {
    if cancelled {
        return Err("worker-cancellation-requested");
    }
    if *phase != "started-held" {
        return Err("worker-not-started-held");
    }
    *phase = "released";
    Ok(())
}
