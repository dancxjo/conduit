//! Host-owned make/save grants and retained manufacture custody, off kernel poll.
use super::policy::Policy;
use conduit::creche_encounter::{
    CrecheEncounterController, CrecheMakeRefusal, CrecheReviewedHandoff,
};
use conduit::creche_production::{CrecheImageProduction, CrecheProducedImages};
use conduit_body_make::DeploymentImageSource;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc, Mutex},
};

#[derive(Default)]
pub(super) struct Manufacture {
    pub job: Option<Box<CrecheImageProduction>>,
    pub refusal: Option<String>,
}
pub(super) type Custody = Arc<Mutex<Manufacture>>;

pub(super) fn produce(
    handoff: CrecheReviewedHandoff,
    cancelled: Arc<AtomicBool>,
    mut policy: Policy,
    directory: PathBuf,
    custody: Custody,
) -> Result<CrecheProducedImages, CrecheMakeRefusal> {
    let source = handoff.draft().image_source();
    let hosts: Vec<_> = match handoff.draft().scope() {
        conduit_body_make::DeploymentScope::FirstHost => {
            vec![handoff.draft().first_host().to_owned()]
        }
        conduit_body_make::DeploymentScope::AllHosts => handoff
            .draft()
            .construction()
            .hosts()
            .iter()
            .map(|h| h.description.name.clone())
            .collect(),
    };
    // Missing acquisition authority is a classified refusal before birth.
    let missing = hosts.iter().any(|host| match source {
        DeploymentImageSource::ReviewedRelease => !policy.reviewed_images.contains_key(host),
        DeploymentImageSource::SourceBuild => !policy.source_builds.contains_key(host),
    });
    if missing {
        let reason = match source {
            DeploymentImageSource::ReviewedRelease => CrecheMakeRefusal::ArtifactRejected,
            DeploymentImageSource::SourceBuild => CrecheMakeRefusal::SourceUnavailable,
        };
        if let Ok(mut retained) = custody.lock() {
            retained.refusal =
                Some("selected Hosts lack explicit image/source grants; no birth performed".into());
        }
        return Err(reason);
    }

    let mut retained = custody.lock().map_err(|_| CrecheMakeRefusal::BuildFailed)?;
    let operation = (|| -> Result<CrecheProducedImages, String> {
        let occurrence = serde_json::to_vec(&serde_json::json!({
            "host":handoff.context().host().host_id,
            "boot":handoff.context().host().boot_id,
            "play":handoff.context().play(), "request":handoff.request().0,
        }))
        .map_err(|e| e.to_string())?;
        let birth_sign = conduit_core::SignId::from(format!(
            "creche/birth/sha256:{:x}",
            Sha256::digest(&occurrence)
        ));
        let job = CrecheImageProduction::prepare(
            handoff,
            0,
            birth_sign,
            policy.maximum_total_bytes,
            cancelled,
        )
        .map_err(|e| format!("prepare: {e:?}"))?;
        retained.job = Some(Box::new(job));
        let job = retained.job.as_mut().ok_or("manufacture custody absent")?;
        // Before manufacturing any media, retain the one actual ordinary birth.
        super::super::write_json_atomic(&directory.join("birth.json"), job.biography())?;
        let parent: serde_json::Value =
            serde_json::from_slice(&occurrence).map_err(|e| e.to_string())?;
        super::super::write_json_atomic(&directory.join("parent-operation.json"), &parent)?;
        let hosts = job.selected_hosts().to_vec();
        for host in hosts {
            let input = match source {
                DeploymentImageSource::ReviewedRelease => policy
                    .reviewed_images
                    .remove(&host)
                    .ok_or("no admitted reviewed image for selected Host")?
                    .open()?,
                DeploymentImageSource::SourceBuild => policy
                    .source_builds
                    .remove(&host)
                    .ok_or("no admitted source build for selected Host")?
                    .grant(),
            };
            let file = new_output(&directory.join(format!("{}.iso", host_key(&host))))?;
            let artifact = job
                .produce_host(&host, input, file)
                .map_err(|e| format!("make: {e:?}"))?;
            super::super::write_json_atomic(
                &directory.join(format!("{}.receipt.json", host_key(&host))),
                artifact.receipt(),
            )?;
            if let Some(builder) = artifact.builder_receipt() {
                super::super::write_json_atomic(
                    &directory.join(format!("{}.builder.json", host_key(&host))),
                    builder,
                )?;
            }
        }
        match retained
            .job
            .take()
            .ok_or("manufacture custody absent")?
            .finish()
        {
            Ok(products) => Ok(products),
            Err((error, job)) => {
                retained.job = Some(job);
                Err(format!("finish: {error:?}"))
            }
        }
    })();
    operation.map_err(|error| {
        retained.refusal = Some(error);
        CrecheMakeRefusal::BuildFailed
    })
}

pub(super) fn service_images(
    controller: &CrecheEncounterController,
    save_directory: &Path,
    operation_directory: &Path,
    maximum_bytes: u64,
) -> Result<(), String> {
    if let Some(pending) = controller.pending_save()? {
        let name = format!(
            "{}-{}.iso",
            host_key(pending.host_entry()),
            crate::birth_identity::fresh_uuid()?
        );
        let destination = save_directory.join(&name);
        let output = new_output(&destination)?;
        let grant_record = serde_json::json!({"host_entry":pending.host_entry(), "destination":destination, "expected_artifact":pending.receipt(), "saved":false});
        super::super::write_json_atomic(
            &operation_directory.join(format!("save-{name}.json")),
            &grant_record,
        )?;
        controller
            .start_save(output, maximum_bytes)
            .map_err(|(error, _)| error)?;
    } else if controller.finish_images_pending()? {
        controller.start_finish_images()?;
    }
    Ok(())
}
pub(super) fn host_key(host: &str) -> String {
    format!("{:x}", Sha256::digest(host.as_bytes()))
}
pub(super) fn new_output(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|e| format!("create native Crèche output: {e}"))
}
