use super::sound_contracts_with_revisions;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use conduit_core::KindContractRevision;
use conduit_form::{KindDefinition, KindSignature};

/// Installs the four portable semantic leaves. This installs no Host offer:
/// availability and implementation remain separate realization facts.
pub fn install_sound_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (contract, revision) in sound_contracts_with_revisions() {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
