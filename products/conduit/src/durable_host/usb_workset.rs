//! CLI carriage entrance; the live owner constructs and signs its own workset.
use super::bounded_read;
use std::path::Path;
pub(crate) fn prepare_usb(
    port: &Path,
    member: &Path,
    source: &Path,
    entries: Vec<String>,
    native_capabilities: Vec<String>,
    state: &Path,
    authorized: bool,
) -> Result<(), String> {
    if !authorized {
        return Err("USB workset requires --authorize-preparation".into());
    }
    let member: crate::durable_host::owner::MemberPresentationBasis =
        serde_json::from_slice(&bounded_read(member, 128 * 1024)?)
            .map_err(|e| format!("member presentation: {e}"))?;
    let source =
        String::from_utf8(bounded_read(source, 256 * 1024)?).map_err(|_| "Source must be UTF-8")?;
    let report = crate::durable_host_control::prepare_usb(
        state,
        port.to_owned(),
        member,
        source,
        entries,
        native_capabilities,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}
