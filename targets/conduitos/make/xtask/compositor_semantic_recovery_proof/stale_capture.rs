//! Capture a completed real refusal frame before acknowledging its checkpoint.
use super::super::{hid_qmp, live_serial, qmp, qmp_display};
use super::{refusal, ConduitosError};
use serde_json::Value;
use std::{
    path::Path,
    process::Child,
    thread,
    time::{Duration, Instant},
};

const PREFIX: &str = "CONDUIT_COMPOSITOR_STALE_REFUSAL ";
const ACK: &str = "CONDUIT_COMPOSITOR_STALE_CAPTURE_ACK";

fn expected(
    face: &str,
    revision: u64,
    old: &str,
    current: &str,
    pixels: u64,
    width: u64,
    height: u64,
) -> Value {
    serde_json::json!({"schema":"conduit.conduitos/stale-show-framebuffer-fixture@1",
        "refusal":"StaleShow","notice":"Stale: refresh","face_id":face,
        "face_revision":revision,"old_show_id":old,"current_show_id":current,
        "same_face":true,"same_draft":true,"current_show_preserved":true,
        "keyboard_available":true,"owner_actions":0,"pixels_written":pixels,
        "width":width,"height":height,"mask_plan_id":"plan/example","active_play_id":"play/example",
        "fixture":true,"physical_scanout":false})
}

fn observation(transcript: &str) -> Result<Option<Value>, ConduitosError> {
    let fail = || {
        refusal("stale-frame-evidence", "requires one actual StaleShow refusal on an unchanged Face/draft with preserved fresh Show and completed framebuffer copy")
    };
    let mut records = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX));
    let Some(record) = records.next() else {
        return Ok(None);
    };
    let value: Value = serde_json::from_str(record).map_err(|_| fail())?;
    let string = |key: &str| {
        value[key]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 512)
            .ok_or_else(fail)
    };
    let number = |key: &str| value[key].as_u64().ok_or_else(fail);
    let face = string("face_id")?;
    let old = string("old_show_id")?;
    let current = string("current_show_id")?;
    let revision = number("face_revision")?;
    let pixels = number("pixels_written")?;
    let width = number("width")?;
    let height = number("height")?;
    let mut wanted = expected(face, revision, old, current, pixels, width, height);
    wanted["mask_plan_id"] = Value::from(string("mask_plan_id")?);
    wanted["active_play_id"] = Value::from(string("active_play_id")?);
    if records.next().is_some()
        || old == current
        || pixels == 0
        || width == 0
        || width > 4096
        || height == 0
        || height > 2160
        || value != wanted
    {
        return Err(fail());
    }
    Ok(Some(value))
}

pub(super) fn capture(
    socket: &Path,
    serial: &Path,
    directory: &Path,
    child: &mut Child,
    deadline: Instant,
) -> Result<Value, ConduitosError> {
    let trace = directory.join("stale-capture-qmp.log");
    let (mut stream, mut reader) = qmp::connect_traced(socket, child, Some(&trace))?;
    let observed = loop {
        let transcript = match live_serial::read_complete(serial) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(refusal("stale-serial-read", error.to_string())),
        };
        if let Some(value) = observation(&transcript)? {
            break value;
        }
        if child
            .try_wait()
            .map_err(|e| refusal("stale-guest-wait", e.to_string()))?
            .is_some()
        {
            return Err(refusal("stale-frame-guest-exited", transcript));
        }
        if Instant::now() >= deadline {
            return Err(refusal("stale-frame-timeout", transcript));
        }
        thread::sleep(Duration::from_millis(10));
    };
    // One capture, no screenshot retries. Guest blocks on real input after
    // the completed notice copy, so no rendering sleeps or timing guesses.
    let (frame, health) =
        qmp_display::capture(&mut stream, &mut reader, directory, "stale-show-refused")?;
    if let Some(error) = health {
        return Err(error);
    }
    if frame["width"] != observed["width"] || frame["height"] != observed["height"] {
        return Err(refusal(
            "stale-frame-extent",
            "capture must match actual framebuffer",
        ));
    }
    hid_qmp::send_named_keys(
        &mut stream,
        &mut reader,
        &["ret"],
        true,
        "stale-capture-ack-down",
    )?;
    hid_qmp::send_named_keys(
        &mut stream,
        &mut reader,
        &["ret"],
        false,
        "stale-capture-ack-up",
    )?;
    Ok(serde_json::json!({"guest":observed,"capture":frame,
        "qmp_transcript_sha256":super::super::report::sha256_file(&trace)?,
        "checkpoint_ack":"real-ps2-enter-after-capture","ordinary_product":false}))
}

pub(super) fn validate_final(transcript: &str, capture: &Value) -> Result<(), ConduitosError> {
    if !transcript.ends_with('\n')
        || observation(transcript)?.as_ref() != Some(&capture["guest"])
        || transcript.lines().filter(|line| *line == ACK).count() != 1
    {
        return Err(refusal("stale-capture-ack-missing", "requires the same unique refusal and actual guest capture acknowledgement before fresh source acceptance"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_show_identity_refusal_and_nonclaims_are_exact() {
        let value = expected(
            "face/exact",
            7,
            "show/old",
            "show/current",
            307200,
            640,
            480,
        );
        let line = format!("{PREFIX}{value}\n");
        assert_eq!(observation(&line).unwrap(), Some(value.clone()));
        assert!(observation(&(line.clone() + &line)).is_err());
        for key in value.as_object().unwrap().keys() {
            let mut bad = value.clone();
            bad[key] = Value::Null;
            assert!(
                observation(&format!("{PREFIX}{bad}\n")).is_err(),
                "missing {key}"
            );
        }
        for (key, replacement) in [
            ("old_show_id", value["current_show_id"].clone()),
            ("owner_actions", Value::from(1)),
            ("pixels_written", Value::from(0)),
            ("physical_scanout", Value::Bool(true)),
            ("width", Value::from(0)),
            ("refusal", Value::from("StaleFace")),
        ] {
            let mut bad = value.clone();
            bad[key] = replacement;
            assert!(observation(&format!("{PREFIX}{bad}\n")).is_err());
        }
        let capture = serde_json::json!({"guest":value});
        assert!(validate_final(&line, &capture).is_err());
        assert!(validate_final(&format!("{line}{ACK}\n"), &capture).is_ok());
        assert!(validate_final(&format!("{line}{ACK}"), &capture).is_err());
        assert!(validate_final(&format!("{line}{ACK}\n{ACK}\n"), &capture).is_err());
    }
}
