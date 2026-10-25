//! Foreground installed Host lifetime under its existing ownership locks.
use super::*;

pub(super) fn run(state_dir: &Path, creche: Option<&Path>) -> Result<(), String> {
    #[cfg(not(all(feature = "native-creche-sdl", target_os = "linux")))]
    if creche.is_some() {
        return Err("this installed product has no native Linux Crèche feature".into());
    }
    use startup_trace::Phase;
    let trace = startup_trace::Trace::from_environment();
    let _ownership = trace.run(Phase::ServiceLock, || owner_lock::acquire(state_dir))?;
    let _body_ownership = trace.run(Phase::BodyLock, || owner_lock::body(state_dir))?;
    trace.run(Phase::Recovery, || owner::recover_retained_state(state_dir))?;
    let owns_body = trace.run(Phase::OwnershipSelection, || {
        read_installation(&state_dir.join("installation.json"))
            .map(|installation| installation.body_state.is_some())
    })?;
    if creche.is_some() && !owns_body {
        return Err("native Crèche requires an existing owned Body".into());
    }
    // Routed admission enters through authenticated local control while the
    // service owns this lock. No second process may edit its Body biography.
    let (status, mut truth) = runtime_preparation::prepare_traced_runtime(state_dir, &trace)?;
    let runtime = state_dir.join("runtime.json");
    if owns_body {
        truth = match trace.run(Phase::OwnedBody, || truth.with_owned_body(state_dir)) {
            Ok(truth) => truth,
            Err(error) => {
                let _ = fs::remove_file(&runtime);
                return Err(error);
            }
        };
    }
    trace.prepared();
    println!(
        "durable host {} boot {} is running",
        status.host_id, status.boot_id
    );
    let outcome = match creche {
        #[cfg(all(feature = "native-creche-sdl", target_os = "linux"))]
        Some(grants) => truth.run_native_creche(state_dir, grants),
        #[cfg(not(all(feature = "native-creche-sdl", target_os = "linux")))]
        Some(_) => unreachable!("native feature checked before preparation"),
        None => crate::durable_host_control::serve(state_dir, truth),
    };
    match fs::remove_file(&runtime) {
        Ok(()) => outcome,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => outcome,
        Err(error) => Err(format!(
            "durable host stopped but its runtime marker could not be retired: {error}"
        )),
    }
}
