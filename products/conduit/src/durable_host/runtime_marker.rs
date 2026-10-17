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
        || status.offer_generation >= next.0
    {
        return Err("durable-host-marker-stale-generation".into());
    }
    status.offer_generation = next.0;
    write_json_atomic(&path, &status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_back_changes_can_advance_more_than_one_generation() {
        let root = std::env::temp_dir().join(format!(
            "conduit-todo-runtime-marker-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let host = HostId::from("host/todo-marker");
        let boot = BootId::from("boot/todo-marker");
        write_json_atomic(
            &root.join("runtime.json"),
            &RuntimeStatus {
                schema: RUNTIME_SCHEMA.into(),
                host_id: host.as_str().into(),
                boot_id: boot.as_str().into(),
                offer_generation: 1,
                process_id: std::process::id(),
                body_id: None,
                release_bundle_sha256: format!("sha256:{}", "0".repeat(64)),
            },
        )
        .unwrap();
        refresh_offer_generation(&root, &host, &boot, OfferGeneration(3)).unwrap();
        let status: RuntimeStatus =
            serde_json::from_slice(&std::fs::read(root.join("runtime.json")).unwrap()).unwrap();
        assert_eq!(status.offer_generation, 3);
        assert!(refresh_offer_generation(&root, &host, &boot, OfferGeneration(3)).is_err());
        assert!(refresh_offer_generation(&root, &host, &boot, OfferGeneration(2)).is_err());
        assert!(refresh_offer_generation(
            &root,
            &host,
            &BootId::from("boot/other"),
            OfferGeneration(4)
        )
        .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
