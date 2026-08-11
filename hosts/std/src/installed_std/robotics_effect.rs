//! Rendering for the observable, effect-free PREWAKE drive projection.

use conduit_core::Scalar;
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SimulatedDriveEffect {
    Projected { linear: Scalar, angular: Scalar },
    Suppressed,
    Cancelled,
}

pub(super) fn write_simulated_drive_effect<W: Write>(
    output: &mut W,
    effect: Option<SimulatedDriveEffect>,
) -> Result<(), String> {
    match effect {
        Some(SimulatedDriveEffect::Projected { linear, angular }) => writeln!(
            output,
            "PREWAKE simulated drive projection linear-microunits={} angular-microunits={} physical-effect=false authority-grant=false",
            linear.raw_microunits(),
            angular.raw_microunits(),
        ),
        Some(SimulatedDriveEffect::Suppressed) => writeln!(
            output,
            "PREWAKE simulated drive suppressed physical-effect=false authority-grant=false"
        ),
        Some(SimulatedDriveEffect::Cancelled) => writeln!(
            output,
            "PREWAKE simulated drive cancelled physical-effect=false authority-grant=false"
        ),
        None => Ok(()),
    }
    .map_err(|error| error.to_string())
}
