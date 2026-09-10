//! A separate fresh boot proves pointer activation, without replacing F10 proof.
use super::super::{hid_qmp, journey_input, profile::Paths, qmp, report::git_head, ConduitosError};
use std::{
    fs,
    os::unix::net::UnixStream,
    path::Path,
    process::{Child, Command, Stdio},
};

const READY: &str = "CONDUIT_BOOT_STAGE pointer-awaiting-report";
const RUN: &str = "CONDUIT_TOUR_CHECKPOINT run-button-completed";
const RELEASE: &str = "CONDUIT_TOUR_CHECKPOINT surface-gesture-released";
const DISPLAY_WIDTH: u16 = 1280;
const DISPLAY_HEIGHT: u16 = 800;
const STATUS_HEIGHT: u16 = 64;
const LESSON_CONTROL_BAR_HEIGHT: u16 = 40;
const SELECTED_NARRATIVE_PERCENT: u16 = 30;
const POINTER_CENTER_NORMALIZED: i64 = 500_000;
const POINTER_NORMALIZED_SCALE: i64 = 4_000;
const POINTER_NORMALIZED_MAX: i64 = 1_000_000;

pub(super) fn execute(paths: &Paths, image: &Path, digest: &str) -> Result<(), ConduitosError> {
    // Keep each boot's evidence; do not overwrite another live proof's sockets.
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io)?
        .as_nanos();
    let directory = paths.target.join(format!("run-control-{epoch}"));
    fs::create_dir(&directory).map_err(io)?;
    let serial = directory.join("serial.log");
    // Unix socket paths must fit sockaddr_un even in long checkout paths.
    let socket =
        std::env::temp_dir().join(format!("conduit-run-{}-{epoch}.sock", std::process::id()));
    let mut command = Command::new("qemu-system-x86_64");
    command
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            "64M",
            "-smp",
            "1",
            "-display",
            "none",
            "-vga",
            "std",
            "-monitor",
            "none",
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "qemu-xhci,id=xhci,p2=3,p3=0",
            "-device",
            "usb-kbd,bus=xhci.0,port=1",
            "-device",
            "usb-mouse,bus=xhci.0,port=2",
        ])
        .arg("-qmp")
        .arg(format!("unix:{},server=on,wait=off", socket.display()))
        .arg("-serial")
        .arg(format!("file:{}", serial.display()))
        .arg("-cdrom")
        .arg(image)
        .args(["-boot", "d"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(directory.join("qemu-stderr.log")).map_err(io)?);
    let argv: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let mut child = command.spawn().map_err(io)?;
    let result = (|| {
        let (mut stream, mut reader) =
            qmp::connect_traced(&socket, &mut child, Some(&directory.join("qmp.log")))?;
        hid_qmp::wait_for_stage(
            &serial,
            &mut child,
            "CONDUIT_BOOT_STAGE front-door-ready",
            "run-control-boot-timeout",
        )?;
        for (key, marker) in [
            ("ret", "\"status\":\"playing\""),
            ("f8", "\"status\":\"stopped\""),
            ("f9", "workspace-opened"),
            ("f11", "chooser-transient-shown"),
        ] {
            let offset = fs::metadata(&serial).map_err(io)?.len() as usize;
            hid_qmp::send_named_keys(&mut stream, &mut reader, &[key], true, "run-control-key")?;
            wait(&serial, &mut child, marker, offset)?;
            hid_qmp::send_named_keys(
                &mut stream,
                &mut reader,
                &[key],
                false,
                "run-control-key-release",
            )?;
        }
        wait(&serial, &mut child, READY, 0)?;
        button(&mut stream, &mut reader, &serial, &mut child, true)?;
        wait(&serial, &mut child, "chooser-gear-selected", 0)?;
        let offset = fs::metadata(&serial).map_err(io)?.len() as usize;
        button(&mut stream, &mut reader, &serial, &mut child, false)?;
        wait(&serial, &mut child, RELEASE, offset)?;
        // After chooser selection the pointer remains centered. Derive the Run
        // target from the selected-workspace layout contract at the graphical
        // product framebuffer size.
        let (dx, dy) = run_pointer_delta(DISPLAY_WIDTH, DISPLAY_HEIGHT)?;
        motion(&mut stream, &mut reader, &serial, &mut child, dx, dy)?;
        button(&mut stream, &mut reader, &serial, &mut child, true)?;
        wait(&serial, &mut child, RUN, 0)?;
        motion(&mut stream, &mut reader, &serial, &mut child, 0, 1)?;
        let offset = fs::metadata(&serial).map_err(io)?.len() as usize;
        button(&mut stream, &mut reader, &serial, &mut child, false)?;
        wait(&serial, &mut child, RELEASE, offset)?;
        let bytes = fs::read(&serial).map_err(io)?;
        validate(journey_input::complete_records(&bytes)?)?;
        let capture = serde_json::json!({"execute":"screendump", "arguments":{
            "filename":directory.join("run-completed.png"), "format":"png"}});
        qmp::request(
            &mut stream,
            &mut reader,
            &serde_json::to_vec(&capture).map_err(io)?,
            "run-control-frame",
        )?;
        let proof = serde_json::json!({"schema":"conduit.conduitos/run-control-proof@1",
            "proof_class":"freestanding-emulator", "source_commit":git_head(&paths.root)?,
            "image_sha256":digest, "qemu_argv":argv, "run_activations":1,
            "held_gesture_did_not_repeat":true, "production_result":"HELLO"});
        fs::write(
            directory.join("proof.json"),
            serde_json::to_vec_pretty(&proof).map_err(io)?,
        )
        .map_err(io)?;
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(socket);
    result?;
    println!("ConduitOS Run control proof: {}", directory.display());
    Ok(())
}

fn wait(
    serial: &Path,
    child: &mut Child,
    marker: &str,
    offset: usize,
) -> Result<(), ConduitosError> {
    journey_input::wait_for_record(serial, child, "run-control-event-timeout", marker, |text| {
        Ok(marker_after(text, marker, offset))
    })
}

// File metadata measures bytes and can stop within a Unicode scalar. Search
// the validated complete-record text by bytes without rounding the offset
// backwards and accidentally accepting a marker from before the action.
fn marker_after(text: &str, marker: &str, offset: usize) -> bool {
    !marker.is_empty()
        && text.as_bytes().get(offset..).is_some_and(|tail| {
            tail.split(|byte| *byte == b'\n').any(|line| {
                line.windows(marker.len())
                    .any(|window| window == marker.as_bytes())
            })
        })
}

fn button(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    down: bool,
) -> Result<(), ConduitosError> {
    let offset = fs::metadata(serial).map_err(io)?.len() as usize;
    journey_input::primary_button(stream, reader, down, "run-control-pointer-button")?;
    wait(serial, child, READY, offset)
}

fn motion(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    x: i64,
    y: i64,
) -> Result<(), ConduitosError> {
    let offset = fs::metadata(serial).map_err(io)?.len() as usize;
    journey_input::relative_motion(stream, reader, x, y, "run-control-pointer-motion")?;
    wait(serial, child, READY, offset)
}

fn run_pointer_delta(width: u16, height: u16) -> Result<(i64, i64), ConduitosError> {
    let run = selected_run_bounds(width, height)?;
    let target_x = i64::from(run.x) + i64::from(run.width / 2);
    let target_y = i64::from(run.y) + i64::from(run.height / 2);
    let target_x_normalized = target_x * POINTER_NORMALIZED_MAX / i64::from(width);
    let target_y_normalized = target_y * POINTER_NORMALIZED_MAX / i64::from(height);
    Ok((
        (target_x_normalized - POINTER_CENTER_NORMALIZED + POINTER_NORMALIZED_SCALE / 2)
            / POINTER_NORMALIZED_SCALE,
        (target_y_normalized - POINTER_CENTER_NORMALIZED + POINTER_NORMALIZED_SCALE / 2)
            / POINTER_NORMALIZED_SCALE,
    ))
}

fn selected_run_bounds(
    width: u16,
    height: u16,
) -> Result<conduit_presentation::LayoutRect, ConduitosError> {
    let available_height = height
        .checked_sub(STATUS_HEIGHT)
        .ok_or_else(|| ConduitosError::refusal("run-control-layout", "empty viewport height"))?;
    let available_width = width
        .checked_sub(inspector_width(width))
        .ok_or_else(|| ConduitosError::refusal("run-control-layout", "empty viewport width"))?;
    let narrative_width = available_width
        .saturating_mul(SELECTED_NARRATIVE_PERCENT)
        .checked_div(100)
        .ok_or_else(|| ConduitosError::refusal("run-control-layout", "invalid narrative split"))?;
    Ok(conduit_presentation::LayoutRect {
        x: 8,
        y: i16::try_from(
            available_height
                .saturating_sub(LESSON_CONTROL_BAR_HEIGHT)
                .saturating_add(6),
        )
        .unwrap_or(i16::MAX),
        width: (narrative_width.saturating_sub(24) / 2).clamp(1, 112),
        height: 28,
    })
}

fn inspector_width(width: u16) -> u16 {
    (width / 3).max(180)
}

#[cfg(test)]
fn delta_hits(
    bounds: conduit_presentation::LayoutRect,
    width: u16,
    height: u16,
    dx: i64,
    dy: i64,
) -> bool {
    let normalized_x = POINTER_CENTER_NORMALIZED + dx * POINTER_NORMALIZED_SCALE;
    let normalized_y = POINTER_CENTER_NORMALIZED + dy * POINTER_NORMALIZED_SCALE;
    let pixel_x = normalized_x * i64::from(width) / POINTER_NORMALIZED_MAX;
    let pixel_y = normalized_y * i64::from(height) / POINTER_NORMALIZED_MAX;
    pixel_x >= i64::from(bounds.x)
        && pixel_x < i64::from(bounds.x) + i64::from(bounds.width)
        && pixel_y >= i64::from(bounds.y)
        && pixel_y < i64::from(bounds.y) + i64::from(bounds.height)
}

fn validate(text: &str) -> Result<(), ConduitosError> {
    let lines: Vec<_> = text.lines().collect();
    let output = lines
        .iter()
        .position(|line| *line == "CONDUIT_SERIAL_PRESENT HELLO");
    let run = lines.iter().position(|line| *line == RUN);
    if lines.iter().filter(|line| **line == RUN).count() != 1
        || !matches!((output, run), (Some(output), Some(run)) if output < run)
        || !run.is_some_and(|run| lines[run + 1..].contains(&RELEASE))
    {
        return Err(ConduitosError::refusal(
            "run-control-causality-invalid",
            "one production result and one completed activation must precede its consumed release",
        ));
    }
    Ok(())
}

fn io(error: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal("run-control-proof-io", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_offsets_inside_unicode_do_not_lose_later_markers() {
        let text = "é中🦀\ncheckpoint\n";
        for offset in 0.."é中🦀".len() {
            assert!(marker_after(text, "checkpoint", offset));
        }
        assert!(!marker_after(text, "checkpoint", "é中🦀\nc".len()));
        assert!(!marker_after(text, "checkpoint", text.len() + 1));
        assert!(!marker_after(text, "", 0));
        assert!(!marker_after("check\npoint\n", "checkpoint", 0));
    }
    #[test]
    fn run_pointer_delta_targets_current_selected_control() {
        let bounds = selected_run_bounds(DISPLAY_WIDTH, DISPLAY_HEIGHT).unwrap();
        assert_eq!(bounds.x, 8);
        assert_eq!(bounds.y, 702);
        assert_eq!(bounds.width, 112);
        assert_eq!(bounds.height, 28);
        let (dx, dy) = run_pointer_delta(DISPLAY_WIDTH, DISPLAY_HEIGHT).unwrap();
        assert_eq!((dx, dy), (-112, 99));
        assert!(delta_hits(bounds, DISPLAY_WIDTH, DISPLAY_HEIGHT, dx, dy));
    }

    #[test]
    fn result_activation_and_release_are_ordered_and_unique() {
        let valid = format!("CONDUIT_SERIAL_PRESENT HELLO\n{RUN}\n{RELEASE}\n");
        validate(&valid).unwrap();
        for invalid in [
            format!("{valid}{RUN}\n"),
            format!("{RUN}\nCONDUIT_SERIAL_PRESENT HELLO\n{RELEASE}\n"),
            format!("{RELEASE}\nCONDUIT_SERIAL_PRESENT HELLO\n{RUN}\n"),
            format!("{RUN}\n{RELEASE}\n"),
        ] {
            assert!(validate(&invalid).is_err());
        }
    }
}
