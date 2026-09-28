//! Retains exact runtime identities needed for standalone execution artifacts.

use conduit_core::{ActivePlayIdentity, Observation, SignIdentity};

pub(super) struct RuntimeArtifactIdentity {
    pub(super) observations: Vec<Observation>,
    pub(super) active_play: Option<ActivePlayIdentity>,
    pub(super) sign_identities: Vec<SignIdentity>,
}

pub(super) fn from_std_report(
    report: conduit_std_host::StdRunReport,
) -> Result<RuntimeArtifactIdentity, String> {
    let Some(kernel) = report.kernel else {
        return Ok(RuntimeArtifactIdentity {
            observations: report.observations,
            active_play: None,
            sign_identities: Vec::new(),
        });
    };
    let sign_identities = report
        .observations
        .iter()
        .map(|observation| {
            let retained = kernel
                .identity
                .sign_identity(&observation.sign_id)
                .ok_or_else(|| {
                    format!(
                        "runtime omitted exact identity for Sign {}",
                        observation.sign_id.as_str()
                    )
                })?;
            let identity = conduit_core::bind_sign(
                &observation.host_id,
                &observation.boot_id,
                observation.active_play_id.as_ref(),
                retained.sequence,
            );
            if identity.sign_id != observation.sign_id {
                return Err(format!(
                    "runtime retained stale identity for Sign {}",
                    observation.sign_id.as_str()
                ));
            }
            Ok(identity)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(RuntimeArtifactIdentity {
        observations: report.observations,
        active_play: Some(kernel.active_play),
        sign_identities,
    })
}
