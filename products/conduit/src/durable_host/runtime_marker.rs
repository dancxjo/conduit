//! Keep the installed service marker consistent with its one live Host's
//! published offer generation as a selected local provider comes and goes.

use super::{bounded_read, write_json_atomic, RuntimeStatus, RUNTIME_SCHEMA};
use conduit_core::{BootId, HostId, OfferGeneration};
use std::path::Path;

pub(crate) fn refresh_offer_generation(
    state_dir: &Path,
    host: &HostId,
    boot: &BootId,
    next: OfferGeneration,
) -> Result<(), String> {
    let path = state_dir.join("runtime.json");
    let bytes = bounded_read(&path, 64 * 1024)?;
    let mut status: RuntimeStatus = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode durable Host runtime marker: {error}"))?;
    if status.schema != RUNTIME_SCHEMA
        || status.host_id != host.as_str()
        || status.boot_id != boot.as_str()
        || status.process_id != std::process::id()
        || status.offer_generation.checked_add(1) != Some(next.0)
    {
        return Err("durable-host-marker-stale-generation".into());
    }
    status.offer_generation = next.0;
    write_json_atomic(&path, &status)
}
