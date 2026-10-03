//! One foreground read-only terminal provider of the actual installed owner.
//! The owner alone prepares and acknowledges the ordinary Mask Show.

use std::{
    io::{BufRead, Read, Write},
    path::Path,
};

const MAX_COMMAND_BYTES: u64 = 256;

pub(crate) fn run(
    state_dir: &Path,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    let attached =
        crate::durable_host_control::terminal_attach::attach_and_show(state_dir, output)?;
    writeln!(
        output,
        "\r\nOwner terminal Show {} · route Plan {} · Host {} · Boot {} · offer generation {} · {} bytes written and flushed. This attached route is read-only; enter quit to detach.",
        attached.show.show_id.as_str(),
        attached.route_plan_id.as_str(),
        attached.advertisement.host_id.as_str(),
        attached.advertisement.boot_id.as_str(),
        attached.advertisement.offer_generation.0,
        attached.effect.bytes_written,
    )
    .map_err(|error| format!("write owner terminal receipt: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("flush owner terminal receipt: {error}"))?;
    loop {
        let mut bytes = Vec::new();
        let length = (&mut *input)
            .take(MAX_COMMAND_BYTES + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|error| format!("read owner terminal command: {error}"))?;
        if length == 0 || bytes == b"quit\n" || bytes == b"quit\r\n" {
            let attached_host = attached.advertisement.host_id.clone();
            let attached_boot = attached.advertisement.boot_id.clone();
            let attached_generation = attached.advertisement.offer_generation;
            drop(attached);
            let (_, retired) = crate::durable_host_control::local_face_snapshot(state_dir)?;
            if retired.host_id != attached_host
                || retired.boot_id != attached_boot
                || retired.offer_generation <= attached_generation
            {
                return Err("owner terminal detachment was not acknowledged".into());
            }
            return Ok(());
        }
        if length as u64 > MAX_COMMAND_BYTES {
            return Err("owner terminal command exceeds 256 bytes".into());
        }
        writeln!(
            output,
            "This owner terminal route is read-only; enter quit to detach."
        )
        .map_err(|error| format!("write owner terminal help: {error}"))?;
    }
}
