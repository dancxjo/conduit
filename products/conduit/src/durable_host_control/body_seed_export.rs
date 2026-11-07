//! Explicit authenticated local export of an already-prepared joining Seed.
//! This carries retained bytes only; no Face path input, Boot or join is implied.
use super::{body::HostSource, DurableHostRuntime};
use conduit_host_make::DeploymentRealizationReceipt;
use std::path::Path;

impl DurableHostRuntime {
    pub(super) fn export_owned_seed(
        &mut self,
        destination: &Path,
    ) -> Result<DeploymentRealizationReceipt, String> {
        let HostSource::Body { owner, running, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        // This synchronous carrier must never occupy an active Body event loop.
        // Check independently of the authenticated dispatcher's same gate.
        if running.is_some() {
            return Err("body-play-active".into());
        }
        owner.export_prepared_seed(destination)
    }
}

#[cfg(unix)]
pub(crate) fn export_owned_seed(state_dir: &Path, destination: &Path) -> Result<(), String> {
    use super::{
        body::{call, token},
        Request, Response, PROTOCOL,
    };
    // Resolve only this explicit CLI argument; the owner receives an absolute
    // destination and never interprets a path from a Face or remote browser.
    let destination = if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| "resolve explicit Seed destination failed")?
            .join(destination)
    };
    // Keep the existing finite local control deadline. A lost reply is unknown;
    // the owner may have completed the carrier, so never remove its destination.
    match call(
        state_dir,
        Request::BodyExportSeed {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            destination,
        },
    )? {
        Response::BodySeedExported {
            protocol: PROTOCOL,
            receipt,
        } => {
            println!(
                "{}",
                serde_json::to_string(&receipt).map_err(|_| "encode Seed export receipt failed")?
            );
            Ok(())
        }
        Response::Refused { code, .. } => Err(format!("Body owner refused Seed export: {code}")),
        _ => Err("Body owner returned the wrong Seed export response".into()),
    }
}
#[cfg(not(unix))]
pub(crate) fn export_owned_seed(_: &Path, _: &Path) -> Result<(), String> {
    Err("Seed export requires authenticated local Unix owner control".into())
}

#[cfg(test)]
mod tests;
