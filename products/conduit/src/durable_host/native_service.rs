//! Retain actual KMSDRM/Mask ownership with optional IMAGE-selected playback.
//! Initialization does not bind a Show or admit per-Plan native rendering.
use super::*;
use startup_trace::{Phase, Trace};
pub(super) fn run_if_selected(
    state_dir: &Path,
    trace: &Trace,
    owns_body: bool,
) -> Result<Option<()>, String> {
    let installation = read_installation(&state_dir.join("installation.json"))?;
    let Some(selection) = installation.selected_host_configuration.as_ref() else {
        return Ok(None);
    };
    let Some(parameters) = selection.scanout_parameters()? else {
        return Ok(None);
    };
    conduit_linux_display::with_kmsdrm(
        parameters.width(),
        parameters.height(),
        |scanout, events| {
            let result = (|| {
                let (status, mut runtime) =
                    runtime_preparation::prepare_traced_runtime_with_scanout(
                        state_dir, trace, scanout,
                    )?;
                if owns_body {
                    trace.run(Phase::OwnedBody, || runtime.attach_owned_body(state_dir))?;
                }
                trace.prepared();
                println!(
                    "durable host {} boot {} is running",
                    status.host_id, status.boot_id
                );
                let mut progress =
                    |_: &mut crate::durable_host_control::DurableHostRuntime| match scanout
                        .poll_output_lifecycle(events, 16)
                        .map_err(|_| "native output lifetime lost")?
                    {
                        conduit_linux_display::NativeInput::Quit => {
                            Err("native output lifetime ended".into())
                        }
                        _ => Ok(()),
                    };
                crate::durable_host_control::serve_with_foreground(
                    state_dir,
                    runtime,
                    &mut progress,
                )
            })();
            match fs::remove_file(state_dir.join("runtime.json")) {
                Ok(()) => result,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => result,
                Err(_) => Err("native service runtime marker could not be retired".into()),
            }
        },
    )?;
    Ok(Some(()))
}
