//! Project a retained emulator snapshot through the current shared Patchbay model.
//!
//! The retired native shell was only a CLI wrapper around this same projection.
//! Product proof must not rebuild or invoke that removed application.

use std::path::Path;

use conduit_patchbay_workbench::PatchbayTopology;

use super::ConduitosError;

pub(super) fn render(snapshot: &Path, code: &'static str) -> Result<String, ConduitosError> {
    let project = || -> Result<String, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(snapshot)?;
        let snapshot = serde_json::from_slice(&bytes)?;
        let mut topology = PatchbayTopology::new(1)?;
        topology.ingest(&snapshot)?;
        Ok(topology.document(None)?.lines().join("\n"))
    };
    project().map_err(|error| ConduitosError::refusal(code, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_projection_accepts_exact_snapshot_and_refuses_wrong_schema() {
        let path =
            std::env::temp_dir().join(format!("conduitos-patchbay-{}.json", std::process::id()));
        let mut snapshot = serde_json::json!({
            "schema": conduit_observatory::SNAPSHOT_SCHEMA,
            "hosts": [], "bases": [], "lines": [], "plans": [], "plays": [],
            "observations": [], "historical_observations": [],
            "sealed_boot_provenance": [],
            "retention": {"item_capacity": 1, "retained_items": 0, "dropped_items": 0}
        });
        std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        let linear = render(&path, "invalid-product-snapshot").unwrap();
        assert!(linear.contains("BASES 0"));
        snapshot["schema"] = "invalid".into();
        std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        assert!(render(&path, "invalid-product-snapshot").is_err());
        std::fs::remove_file(path).unwrap();
    }
}
