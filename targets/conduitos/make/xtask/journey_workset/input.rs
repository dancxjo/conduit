//! Real QMP input; observations never perform or retry an action.
use std::{os::unix::net::UnixStream, path::Path, process::Child};

use super::super::{
    hid_qmp, journey_input, journey_records, qemu_artifacts::Artifacts, qmp, ConduitosError,
};
use super::{identity, matches, Expected, NativePlot};

pub(in super::super) fn exercise(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    artifacts: &mut Artifacts,
) -> Result<(), ConduitosError> {
    use NativePlot::{KeyboardCanvas as Canvas, MemoryLantern as Memory, Patchbay, Tour};
    let mut foreground = Canvas;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Patchbay,
        10,
        None,
    )?;
    artifacts.capture(stream, reader, "patchbay-current-canvas", true)?;
    application_action(stream, reader, serial, child, "f10", Patchbay, 11)?;
    application_action(stream, reader, serial, child, "f11", Patchbay, 12)?;
    artifacts.capture(stream, reader, "patchbay-edit-requested", true)?;
    application_action(stream, reader, serial, child, "f1", Patchbay, 13)?;
    artifacts.capture(stream, reader, "patchbay-presenters-replanned", true)?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Tour,
        13,
        None,
    )?;
    application_action(stream, reader, serial, child, "f10", Tour, 14)?;
    artifacts.capture(stream, reader, "resident-tour-result", true)?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Memory,
        14,
        None,
    )?;
    for (key, count, result) in [("o", 16, "o"), ("n", 18, "on"), ("e", 20, "one")] {
        pair(stream, reader, serial, child, key, Memory, count, result)?;
    }
    artifacts.capture(stream, reader, "memory-listening", true)?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Canvas,
        20,
        Some("HELLO"),
    )?;
    artifacts.capture(stream, reader, "canvas-retained", true)?;
    hid_qmp::send_named_keys(stream, reader, &["x"], true, "workset-held-press")?;
    wait(serial, child, Canvas, 21, Some("HELLOX"))?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Patchbay,
        21,
        None,
    )?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Tour,
        21,
        None,
    )?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Memory,
        21,
        Some("one"),
    )?;
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
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Canvas,
        32,
        Some("HELLOX"),
    )?;
    pair(stream, reader, serial, child, "y", Canvas, 34, "HELLOXY")?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Patchbay,
        34,
        None,
    )?;
    artifacts.capture(stream, reader, "patchbay-current-canvas-returned", true)?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Tour,
        34,
        None,
    )?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Memory,
        34,
        Some("hi"),
    )?;
    artifacts.capture(stream, reader, "memory-retained", true)?;
    switch(
        stream,
        reader,
        serial,
        child,
        &mut foreground,
        Canvas,
        34,
        Some("HELLOXY"),
    )
}

fn application_action(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    key: &str,
    plot: NativePlot,
    count: u64,
) -> Result<(), ConduitosError> {
    journey_input::key_pair(stream, reader, key, "workset-application-action")?;
    wait(serial, child, plot, count, None)
}

fn wait(
    serial: &Path,
    child: &mut Child,
    plot: NativePlot,
    count: u64,
    result: Option<&'static str>,
) -> Result<(), ConduitosError> {
    let identity = identity(plot)?;
    journey_input::wait_for_record(
        serial,
        child,
        "product-journey-workset-input-timeout",
        plot.title(),
        |text| {
            // Every expected (Plot, input count, result, identity) tuple is
            // unique in this journey. A later projection may follow the
            // checkpoint before the observer reads the append-only log, so
            // validate the emitted checkpoint rather than only the tail.
            Ok(journey_records::decode(text)?.iter().any(|record| {
                matches(
                    record,
                    Expected {
                        plot,
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
    current: &mut NativePlot,
    plot: NativePlot,
    count: u64,
    result: Option<&'static str>,
) -> Result<(), ConduitosError> {
    let target = identity(plot)?;
    for _ in 0..conduitos::native_workset::NATIVE_PLOT_CAPACITY {
        let before = std::fs::read_to_string(serial)
            .ok()
            .and_then(|text| journey_records::decode(&text).ok())
            .and_then(|records| {
                records
                    .last()
                    .and_then(|record| record["revision"].as_u64())
            })
            .unwrap_or(0);
        journey_input::key_pair(stream, reader, "tab", "workset-select")?;
        journey_input::wait_for_record(
            serial,
            child,
            "product-journey-workset-input-timeout",
            plot.title(),
            |text| {
                Ok(journey_records::decode(text)?.last().is_some_and(|record| {
                    record["revision"]
                        .as_u64()
                        .is_some_and(|revision| revision > before)
                }))
            },
        )?;
        let text = std::fs::read_to_string(serial).map_err(|error| {
            ConduitosError::refusal("product-journey-serial-unavailable", error.to_string())
        })?;
        if journey_records::decode(&text)?
            .last()
            .is_some_and(|record| {
                record["source_document_id"] == target.source_document_id
                    && record["checked_plot_id"] == target.checked_plot_id
                    && record["expanded_plot_id"] == target.expanded_plot_id
            })
        {
            *current = plot;
            return wait(serial, child, plot, count, result);
        }
    }
    Err(ConduitosError::refusal(
        "product-journey-workset-invalid",
        format!(
            "{} was not found in one complete native inventory cycle",
            plot.title()
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
fn pair(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
    key: &str,
    plot: NativePlot,
    count: u64,
    result: &'static str,
) -> Result<(), ConduitosError> {
    hid_qmp::send_named_keys(stream, reader, &[key], true, "workset-input")?;
    wait(serial, child, plot, count - 1, Some(result))?;
    hid_qmp::send_named_keys(stream, reader, &[key], false, "workset-input")?;
    wait(serial, child, plot, count, Some(result))
}
