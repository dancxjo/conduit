//! The installed screen-free client's acknowledged terminal Show for one Face.

use std::io::Write;

use conduit_presentation::{MaskShow, Presentation};
use conduit_std_host::terminal_face_mask::TerminalFaceMask;
use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;

use super::debug_error;

pub(super) fn present(
    face: &Presentation,
    execution: &mut HostedTerminalMaskExecution,
    output: &mut impl Write,
) -> Result<MaskShow, String> {
    let mut mask = TerminalFaceMask::prepare(face.clone(), 80, 24).map_err(debug_error)?;
    mask.present(execution, output).map_err(debug_error)?;
    writeln!(output).map_err(|error| error.to_string())?;
    mask.show()
        .cloned()
        .ok_or_else(|| "terminal did not acknowledge a Show".into())
}
