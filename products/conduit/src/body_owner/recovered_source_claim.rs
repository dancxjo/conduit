//! A recovered private claim denies effects; its bytes confer no authority.
use super::Owner;
use std::path::Path;

/// No parsing, replay, receiver-retirement claim or resource reclamation.
/// Inspect before restoring any executable owner startup work.
pub(crate) fn inspect(root: &Path) -> Result<bool, String> {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata(root).map_err(|e| e.to_string())?.uid();
    let directory = root.join("source-execution");
    let metadata = match std::fs::symlink_metadata(&directory) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("inspect recovered Source claim directory: {error}")),
    };
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err("recovered Source claim directory is not owner-private".into());
    }
    let marker = match std::fs::symlink_metadata(directory.join("claimed.json")) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("inspect recovered Source claim: {error}")),
    };
    if !marker.is_file() || marker.uid() != uid || marker.mode() & 0o077 != 0 {
        return Err("recovered Source claim is not an owner-private regular file".into());
    }
    Ok(true)
}

impl Owner {
    pub(crate) fn hold_recovered_source_claim(&mut self, held: bool) {
        // Monotonic within this Owner. Neither a viewer return nor file removal
        // can release the hold. A fresh same-process claim does not set it.
        self.recovered_source_claim |= held;
    }
    pub(crate) fn require_source_effects_unheld(&self) -> Result<(), String> {
        if self.recovered_source_claim {
            Err("recovered-source-claim-unresolved: effects remain denied; receiver retirement unproved".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
impl Owner {
    /// Local lifecycle-only fixture. Empty sealed Plans carry the actual
    /// workset identities; this is no physical placement or Play claim.
    pub(crate) fn propose_recovered_claim_test_awake(&mut self, root: &Path) -> Result<(), String> {
        let plans = self
            .session
            .evidence()
            .body
            .workset
            .plots()
            .iter()
            .map(|plot| conduit_body::BodyPlotPlan {
                plot: plot.clone(),
                plan: conduit_core::seal_plan(
                    conduit_core::PlotIdentity {
                        source_document_id: plot.source_document_id.clone(),
                        checked_plot_id: plot.checked_plot_id.clone(),
                        expanded_plot_id: "fixture/recovery-metadata".into(),
                    },
                    vec![],
                ),
            })
            .collect();
        let ad = self.host.advertisement();
        self.session
            .propose(plans, &ad.host_id, &ad.boot_id)
            .map_err(|e| format!("{e:?}"))?;
        self.persist(root)
    }
}

#[cfg(test)]
#[path = "recovered_source_claim/tests.rs"]
mod tests;
