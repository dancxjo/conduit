//! Finite immutable initial encounter admission, distinct from later make grants.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn digest(
    draft: &DeploymentDraft,
    profiles: &[CheckedHostConfiguration],
    encounter: &str,
    pool: &ResourcePoolId,
    limits: CrecheLimits,
    interactions: u32,
    packages: &MakePackageSet,
) -> Result<[u8; 32], String> {
    if profiles.len() > conduit_body_make::MAX_DEPLOYMENT_PROFILES {
        return Err("Crèche profile catalogue exceeds its finite bound".into());
    }
    let mut selections = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let checked = conduit_host_make::check_host_configuration(
            profile.configuration().clone(),
            &conduit_workspace_make::catalog(),
            packages,
        )
        .map_err(|e| format!("Crèche checked profile: {e:?}"))?;
        if checked.profile() != profile.profile()
            || checked.configuration_id() != profile.configuration_id()
        {
            return Err("Crèche profile differs from composed package catalogue".into());
        }
        let output = &packages
            .target_descriptor(&profile.profile().target.key())
            .ok_or("Crèche target absent")?
            .default_output;
        selections.push(
            packages
                .derive_build_selection(profile.profile(), output)
                .map_err(|e| format!("Crèche make selection: {e:?}"))?,
        );
    }
    let configurations = profiles
        .iter()
        .map(|p| p.configuration())
        .collect::<Vec<_>>();
    let initial = serde_json::to_vec(&(
        "conduit.creche/encounter-admission@1",
        encounter,
        pool,
        limits,
        interactions,
        draft.construction().description(),
        draft.birth().friendly_name(),
        draft.birth().choices(),
        draft.birth().requested_system(),
        draft.birth().search(),
        draft.first_host(),
        match draft.scope() {
            conduit_body_make::DeploymentScope::FirstHost => "first",
            _ => "all",
        },
        match draft.image_source() {
            conduit_body_make::DeploymentImageSource::ReviewedRelease => "release",
            _ => "build",
        },
        configurations,
        selections,
    ))
    .map_err(|e| format!("Crèche initial admission: {e}"))?;
    Ok(Sha256::digest(initial).into())
}

pub(super) fn basis(
    encounter_id: &str,
    context: &CrecheExecutionContext,
) -> Result<DeploymentEncounterBasis, String> {
    let partition = context
        .plan()
        .plots
        .iter()
        .find(|p| &p.plot == context.plot())
        .ok_or("Crèche execution lost its exact producing Plot")?;
    Ok(DeploymentEncounterBasis {
        host_id: context.host().host_id.clone(),
        boot_id: context.host().boot_id.clone(),
        encounter_id: encounter_id.into(),
        execution: FaceBasis {
            body_id: Some(context.plan().body_id.clone()),
            wake_id: Some(context.plan().wake_id.clone()),
            source_document_id: Some(partition.plan.source_document_id.clone()),
            checked_plot_id: Some(partition.plan.checked_plot_id.clone()),
            expanded_plot_id: Some(partition.plan.expanded_plot_id.clone()),
            plan_id: Some(partition.plan.plan_id.clone()),
            active_play_id: Some(context.play().active_play_id.clone()),
            sign_ids: vec![],
        },
    })
}
