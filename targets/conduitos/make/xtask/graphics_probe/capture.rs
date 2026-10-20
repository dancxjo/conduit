//! Existing bounded QMP transport and display capture, plus read-only handoff decoding.
use super::super::{qmp, qmp_display};
use super::guest::Guest;
use super::{debugger, fail, io, limine, ConduitosArch, ConduitosError, ProbeArgs};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::Path,
    thread,
    time::{Duration, Instant},
};
pub(super) const HANDOFF_SECONDS: u64 = 60;
const SERIAL_BYTES: usize = 1024 * 1024;

pub(super) fn observe(
    args: &ProbeArgs,
    checkpoint: u64,
    guest: &mut Guest,
    output: &Path,
) -> Result<Value, ConduitosError> {
    let (mut stream, mut reader) = qmp::connect_traced(
        &guest.socket,
        &mut guest.child,
        Some(&output.join("qmp.jsonl")),
    )?;
    let observation = if args.arch == ConduitosArch::Ia32 {
        debugger::observe(guest, checkpoint, output)?
    } else {
        let deadline = Instant::now() + Duration::from_secs(HANDOFF_SECONDS);
        loop {
            let mut serial = Vec::new();
            fs::File::open(&guest.serial)
                .map_err(io)?
                .take((SERIAL_BYTES + 1) as u64)
                .read_to_end(&mut serial)
                .map_err(io)?;
            if serial.len() > SERIAL_BYTES {
                return Err(fail(
                    "graphics-probe-serial-bound",
                    "serial transcript exceeds 1 MiB",
                ));
            }
            if String::from_utf8_lossy(&serial).contains("CONDUIT_") {
                break;
            }
            if let Some(status) = guest.child.try_wait().map_err(io)? {
                return Err(fail("graphics-probe-guest-exited", status.to_string()));
            }
            if Instant::now() >= deadline {
                return Err(fail(
                    "graphics-probe-handoff-timeout",
                    "no kernel checkpoint within the 60 second discovery budget",
                ));
            }
            thread::sleep(Duration::from_millis(20));
        }
        qmp::request(
            &mut stream,
            &mut reader,
            br#"{"execute":"stop"}"#,
            "observe-stop",
        )?;
        let mut transport_error = None;
        let observation = limine::observe(checkpoint, |address, count| {
            match read_words(&mut stream, &mut reader, address, count) {
                Ok(words) => Ok(words),
                Err(error) => {
                    let reason = error.reason;
                    transport_error = Some(error);
                    Err(reason)
                }
            }
        })
        .map_err(|reason| {
            transport_error.unwrap_or_else(|| {
                fail(
                    reason,
                    "guest response cannot be interpreted as the pinned Limine handoff",
                )
            })
        })?;
        json!({"handoff":"limine","request_address":format!("{checkpoint:#x}"),"response_present":observation.response_present,
            "response_revision":observation.response_revision,"framebuffer_count":observation.framebuffers.len(),
            "framebuffers":observation.framebuffers.iter().map(|framebuffer|json!({"virtual_address":format!("{:#x}",framebuffer.virtual_address),
                "current":mode(&framebuffer.current),"modes":framebuffer.modes.iter().map(mode).collect::<Vec<_>>()})).collect::<Vec<_>>()})
    };
    // A uniform or firmware-only display is useful negative G0 evidence. The
    // existing collector reports its health refusal; it never becomes Show proof.
    let display = match qmp_display::capture(&mut stream, &mut reader, output, "observed-display") {
        Ok((frame, health)) => {
            json!({"status":"captured","frame":frame,"health_refusal":health.map(|e|json!({"code":e.reason,"detail":e.detail}))})
        }
        Err(error) => {
            json!({"status":"unavailable","refusal":{"code":error.reason,"detail":error.detail}})
        }
    };
    Ok(
        json!({"handoff":observation,"display":display,"graphical_show_proved":false,"elapsed_to_observed_display_ms":guest.started.elapsed().as_millis()}),
    )
}
fn mode(mode: &limine::Mode) -> Value {
    json!({"width":mode.width,"height":mode.height,"pitch":mode.pitch,"bits_per_pixel":mode.bits_per_pixel,"memory_model":mode.memory_model,"red":mode.red,"green":mode.green,"blue":mode.blue})
}
fn read_words(
    stream: &mut std::os::unix::net::UnixStream,
    reader: &mut qmp::Reader,
    address: u64,
    count: usize,
) -> Result<Vec<u64>, ConduitosError> {
    let command = json!({"execute":"human-monitor-command","arguments":{"command-line":format!("x /{count}gx {address:#x}")}});
    let result = qmp::request_value(
        stream,
        reader,
        command.to_string().as_bytes(),
        "read-handoff",
    )?;
    let text = result
        .as_str()
        .ok_or_else(|| fail("graphics-probe-memory-response", "expected HMP text"))?;
    parse_words(text, count).map_err(|reason| {
        fail(
            reason,
            "debugger did not return the exact bounded word extent",
        )
    })
}
fn parse_words(text: &str, count: usize) -> Result<Vec<u64>, &'static str> {
    if count > 16 || text.len() > 4096 {
        return Err("graphics-probe-memory-bound");
    }
    let mut words = Vec::with_capacity(count);
    for line in text.lines() {
        let Some((_, values)) = line.split_once(':') else {
            continue;
        };
        for token in values.split_whitespace() {
            let Some(value) = token.strip_prefix("0x") else {
                continue;
            };
            if words.len() == count {
                return Err("graphics-probe-memory-count");
            }
            words.push(
                u64::from_str_radix(value, 16).map_err(|_| "graphics-probe-memory-encoding")?,
            );
        }
    }
    if words.len() != count {
        return Err("graphics-probe-memory-count");
    }
    Ok(words)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unreadable_extra_and_malformed_memory_never_become_zero_filled_responses() {
        assert_eq!(
            parse_words("Cannot access memory", 1),
            Err("graphics-probe-memory-count")
        );
        assert_eq!(
            parse_words("0x1000: 0x01 0x02", 1),
            Err("graphics-probe-memory-count")
        );
        assert_eq!(
            parse_words("0x1000: 0xnothex", 1),
            Err("graphics-probe-memory-encoding")
        );
        assert_eq!(
            parse_words("0x1000: 0xffff000059874000 0x03\r\n", 2),
            Ok(vec![0xffff000059874000, 3])
        );
    }
}
