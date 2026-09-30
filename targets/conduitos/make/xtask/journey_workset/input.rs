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
    use NativeForm::{KeyboardCanvas as Canvas, MemoryLantern as Memory, Patchbay, Tour};
    switch(stream, reader, serial, child, Patchbay, 10, None)?;
    artifacts.capture(stream, reader, "patchbay-current-canvas", true)?;
    application_action(stream, reader, serial, child, "f10", Patchbay, 11)?;
    application_action(stream, reader, serial, child, "f11", Patchbay, 12)?;
    artifacts.capture(stream, reader, "patchbay-edit-requested", true)?;
    application_action(stream, reader, serial, child, "f1", Patchbay, 13)?;
    artifacts.capture(stream, reader, "patchbay-presenters-replanned", true)?;
    switch(stream, reader, serial, child, Tour, 13, None)?;
    application_action(stream, reader, serial, child, "f10", Tour, 14)?;
    artifacts.capture(stream, reader, "resident-tour-result", true)?;
    switch(stream, reader, serial, child, Memory, 14, None)?;
    for (key, count, result) in [("o", 16, "o"), ("n", 18, "on"), ("e", 20, "one")] {
        pair(stream, reader, serial, child, key, Memory, count, result)?;
    }
    artifacts.capture(stream, reader, "memory-listening", true)?;
    switch(stream, reader, serial, child, Canvas, 20, Some("HELLO"))?;
    artifacts.capture(stream, reader, "canvas-retained", true)?;
    hid_qmp::send_named_keys(stream, reader, &["x"], true, "workset-held-press")?;
    wait(serial, child, Canvas, 21, Some("HELLOX"))?;
    switch(stream, reader, serial, child, Patchbay, 21, None)?;
    switch(stream, reader, serial, child, Tour, 21, None)?;
    switch(stream, reader, serial, child, Memory, 21, Some("one"))?;
    hid_qmp::send_named_keys(stream, reader, &["x"], false, "workset-held-release")?;
    wait(serial, child, Memory, 22, Some("one"))?;
    for (count, result) in [(24, "on"), (26, "o"), (28, "")] {
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
    pair(stream, reader, serial, child, "h", Memory, 30, "h")?;
    pair(stream, reader, serial, child, "i", Memory, 32, "hi")?;
    switch(stream, reader, serial, child, Canvas, 32, Some("HELLOX"))?;
    pair(stream, reader, serial, child, "y", Canvas, 34, "HELLOXY")?;
    switch(stream, reader, serial, child, Patchbay, 34, None)?;
    artifacts.capture(stream, reader, "patchbay-current-canvas-returned", true)?;
    switch(stream, reader, serial, child, Tour, 34, None)?;
    switch(stream, reader, serial, child, Memory, 34, Some("hi"))?;
    artifacts.capture(stream, reader, "memory-retained", true)?;
    switch(stream, reader, serial, child, Canvas, 34, Some("HELLOXY"))
}

fn application_action(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    key: &str,
    form: NativeForm,
    count: u64,
) -> Result<(), ConduitosError> {
    journey_input::key_pair(stream, reader, key, "workset-application-action")?;
    wait(serial, child, form, count, None)
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
