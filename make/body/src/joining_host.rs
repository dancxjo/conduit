//! Checked construction for another Host of an already-born Body.
//! No birth, invitation issuance, artifact production or admission occurs here.
use std::collections::BTreeMap;

use conduit_body::{BodyBiographyEvidence, SpawnInvitationClaim};
use conduit_host_make::{CheckedHostConfiguration, MakeCatalog, MakePackageSet, SporeOutputKind};

use crate::{
    check_body_description, BodyBindingTarget, BodyDescription, BodyHostDescription,
    BodyWardrobeDescription, CheckedBodyDescription, DeploymentDescription, SporeDescription,
    SporeJoinMode, BODY_DESCRIPTION_SCHEMA,
};

pub struct JoiningHostSelection<'a> {
    pub configuration: &'a CheckedHostConfiguration,
    pub output: SporeOutputKind,
    pub deployment: Option<DeploymentDescription>,
}

/// Recheck the exact Host selection under the supplied catalogue, retaining the
/// existing Body identity and exact public invitation. The caller must separately
/// own invitation issuance/secret provisioning and actual Make/deployment grants.
/// A checked return value grants no authority and proves no image, Boot or join.
pub fn check_joining_host(
    body: &BodyBiographyEvidence,
    invitation: &SpawnInvitationClaim,
    selection: JoiningHostSelection<'_>,
    now_millis: u64,
    catalog: &MakeCatalog,
    packages: &MakePackageSet,
) -> Result<CheckedBodyDescription, String> {
    body.validate()
        .map_err(|error| format!("joining Host Body evidence: {error:?}"))?;
    invitation
        .inspect(now_millis)
        .map_err(|error| format!("joining Host invitation: {error:?}"))?;
    if invitation.body_id != body.body_id {
        return Err("joining Host invitation names a different Body".into());
    }
    if matches!(body.body.state, conduit_body::BodyState::Fulfilled { .. }) {
        return Err("a fulfilled Body cannot acquire another Host".into());
    }
    let configuration = selection.configuration;
    let name = configuration.configuration().name.clone();
    let digest = configuration
        .configuration_id()
        .rsplit(':')
        .next()
        .unwrap_or_default();
    let key = format!("profiles/{digest}.host.conduit");
    let checked = check_body_description(
        BodyDescription {
            schema: BODY_DESCRIPTION_SCHEMA,
            name: body.friendly_name.clone(),
            body: BodyBindingTarget {
                id: body.body_id.as_str().into(),
            },
            hosts: vec![BodyHostDescription {
                name,
                part: None,
                configuration: key.clone(),
                spore: SporeDescription {
                    join_mode: SporeJoinMode::SelfJoining,
                    output: selection.output,
                    invitation: Some(invitation.invitation_id.as_str().into()),
                },
                deployment: selection.deployment,
            }],
            mask_imports: Vec::new(),
            wardrobe: BodyWardrobeDescription::default(),
        },
        &BTreeMap::from([(key, configuration.configuration().clone())]),
        catalog,
        packages,
    )
    .map_err(|errors| format!("joining Host construction: {errors:?}"))?;
    if checked.hosts()[0].configuration.configuration_id() != configuration.configuration_id()
        || checked.hosts()[0].configuration.profile() != configuration.profile()
    {
        return Err("joining Host configuration changed under the selected catalogue".into());
    }
    Ok(checked)
}

#[cfg(test)]
mod tests;
