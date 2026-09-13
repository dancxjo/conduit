//! Real QMP input; observations never perform or retry an action.
use std::{os::unix::net::UnixStream, path::Path, process::Child};

use super::super::{
    hid_qmp, journey_input, journey_records, qemu_artifacts::Artifacts, qmp, ConduitosError,
};
use super::{identity, matches, Expected, NativeForm};

pub(in super::super) fn exercise(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    artifacts: &mut Artifacts,
) -> Result<(), ConduitosError> {
    use NativeForm::{KeyboardCanvas as Canvas, MemoryLantern as Memory};
    switch(stream, reader, serial, child, Memory, 10, None)?;
    for (key, count, result) in [("o", 12, "o"), ("n", 14, "on"), ("e", 16, "one")] {
        pair(stream, reader, serial, child, key, Memory, count, result)?;
    }
    artifacts.capture(stream, reader, "memory-listening", true)?;
    switch(stream, reader, serial, child, Canvas, 16, Some("HELLO"))?;
    artifacts.capture(stream, reader, "canvas-retained", true)?;
    hid_qmp::send_named_keys(stream, reader, &["x"], true, "workset-held-press")?;
    wait(serial, child, Canvas, 17, Some("HELLOX"))?;
    switch(stream, reader, serial, child, Memory, 17, Some("one"))?;
    hid_qmp::send_named_keys(stream, reader, &["x"], false, "workset-held-release")?;
    wait(serial, child, Memory, 18, Some("one"))?;
    for (count, result) in [(20, "on"), (22, "o"), (24, "")] {
        pair(
            stream,
            reader,
            serial,
            child,
            "backspace",
            Memory,
            count,
            result,
        )?;
    }
    artifacts.capture(stream, reader, "memory-cleared", true)?;
    pair(stream, reader, serial, child, "h", Memory, 26, "h")?;
    pair(stream, reader, serial, child, "i", Memory, 28, "hi")?;
    switch(stream, reader, serial, child, Canvas, 28, Some("HELLOX"))?;
    pair(stream, reader, serial, child, "y", Canvas, 30, "HELLOXY")?;
    switch(stream, reader, serial, child, Memory, 30, Some("hi"))?;
    artifacts.capture(stream, reader, "memory-retained", true)?;
    switch(stream, reader, serial, child, Canvas, 30, Some("HELLOXY"))
}

fn wait(
    serial: &Path,
    child: &mut Child,
    form: NativeForm,
    count: u64,
    result: Option<&'static str>,
) -> Result<(), ConduitosError> {
    let identity = identity(form)?;
    journey_input::wait_for_record(
        serial,
        child,
        "product-journey-workset-input-timeout",
        form.title(),
        |text| {
            Ok(journey_records::decode(text)?.last().is_some_and(|record| {
                matches(
                    record,
                    Expected {
                        form,
                        count,
                        result,
                    },
                    &identity,
                )
            }))
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn switch(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    form: NativeForm,
    count: u64,
    result: Option<&'static str>,
) -> Result<(), ConduitosError> {
    journey_input::key_pair(stream, reader, "tab", "workset-select")?;
    wait(serial, child, form, count, result)
}

#[allow(clippy::too_many_arguments)]
fn pair(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    key: &str,
    form: NativeForm,
    count: u64,
    result: &'static str,
) -> Result<(), ConduitosError> {
    hid_qmp::send_named_keys(stream, reader, &[key], true, "workset-input")?;
    wait(serial, child, form, count - 1, Some(result))?;
    hid_qmp::send_named_keys(stream, reader, &[key], false, "workset-input")?;
    wait(serial, child, form, count, Some(result))
}
