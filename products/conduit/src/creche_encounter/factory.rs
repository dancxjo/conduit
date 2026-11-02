use super::*;
use conduit_host_make::{build_host_image, check_host_configuration, BuildInputs};
use std::io::{Read, Seek, SeekFrom};

impl CrecheEncounterFactory {
    pub fn new(
        catalog: MakeCatalog,
        packages: MakePackageSet,
        installation: CrecheEncounterInstallation,
        mut artifacts: Vec<CrecheReviewedArtifact>,
    ) -> Result<Self, String> {
        let b = installation.production_bounds;
        if !installation.limits.valid()
            || installation.encounter_id.is_empty()
            || installation.encounter_id.len() > 128
            || installation.encounter_id.chars().any(char::is_control)
            || installation.resource_pool.as_str().is_empty()
            || b.maximum_artifact_bytes == 0
            || !(1..=1024 * 1024).contains(&b.maximum_manifest_bytes)
            || b.maximum_receipt_bytes < 1024
            || b.maximum_receipt_bytes as u64 > installation.limits.maximum_evidence_bytes
            || installation.limits.maximum_evidence_bytes > 16 * 1024 * 1024
            || artifacts.is_empty()
            || artifacts.len() > 8
        {
            return Err("Crèche installation lacks finite admitted inventory/limits".into());
        }
        // Admit the form and immutable catalogue before publishing any slot.
        if let Some(source) = &installation.initial_source {
            HostConfigurationEncounter::load(source, &catalog, &packages)?;
        } else {
            HostConfigurationEncounter::new(&catalog, &packages)?;
        }
        let mut identities = Vec::with_capacity(artifacts.len());
        for artifact in &mut artifacts {
            if identities
                .iter()
                .any(|id: &(String, SporeOutputKind, String, u64)| {
                    id.0 == artifact.configuration.configuration_id()
                })
            {
                return Err("Crèche reviewed artifact selection is ambiguous".into());
            }
            validate(artifact, &catalog, &packages, b)?;
            identities.push((
                artifact.configuration.configuration_id().into(),
                artifact.output.clone(),
                artifact.artifact_digest.clone(),
                artifact.artifact_bytes,
            ));
        }
        let admission = serde_json::to_vec(&(
            "conduit.creche/installed-inventory@1",
            catalogue_digest(&catalog, &packages),
            &installation.encounter_id,
            &installation.resource_pool,
            installation.limits,
            (
                b.maximum_artifact_bytes,
                b.maximum_manifest_bytes,
                b.maximum_receipt_bytes,
            ),
            &installation.initial_source,
            identities,
        ))
        .map_err(|e| format!("Crèche inventory identity: {e}"))?;
        let resource = conduit_core::resource_offer(
            installation.resource_pool.as_str(),
            conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
            1,
        );
        Ok(Self {
            inputs: Arc::new(Inputs {
                catalog,
                packages,
                installation,
                artifacts,
                resource,
                admission_digest: Sha256::digest(admission).into(),
            }),
        })
    }
    pub fn resource_offer(&self) -> ResourceOffer {
        self.inputs.resource.clone()
    }
    pub fn limits(&self) -> CrecheLimits {
        self.inputs.installation.limits
    }

    pub fn encounter(
        &mut self,
        binding: CrecheBodyBinding,
        custody: Box<dyn CrecheMaskShowCustody>,
    ) -> Result<(CrecheEncounterAdapter<'_>, CrecheEncounterController), String> {
        binding
            .biography
            .validate()
            .map_err(|e| format!("Crèche Body biography: {e:?}"))?;
        binding
            .invitation
            .validate(clock()?)
            .map_err(|e| format!("Crèche invitation: {e:?}"))?;
        if binding.biography.body_id != binding.invitation.claim.body_id {
            return Err("Crèche invitation belongs to another Body".into());
        }
        let encounter = match &self.inputs.installation.initial_source {
            Some(source) => HostConfigurationEncounter::load(
                source,
                &self.inputs.catalog,
                &self.inputs.packages,
            )?,
            None => HostConfigurationEncounter::new(&self.inputs.catalog, &self.inputs.packages)?,
        };
        let channel = Arc::new(Channel {
            state: Mutex::new(State {
                active: false,
                retired: false,
                face: None,
                acknowledged: None,
                command: None,
                checked: None,
                request: None,
                started: false,
                worker: None,
                cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                result: None,
                products: None,
                published: false,
            }),
            custody: Mutex::new(custody),
            inputs: self.inputs.clone(),
            binding,
        });
        let mut evidence = Vec::new();
        evidence
            .try_reserve_exact(self.inputs.installation.limits.maximum_evidence_bytes as usize)
            .map_err(|_| "Crèche retained evidence allocation unavailable")?;
        Ok((
            CrecheEncounterAdapter {
                encounter,
                channel: channel.clone(),
                basis: None,
                request: None,
                evidence,
                outcome: None,
                begun: false,
                retired: false,
            },
            CrecheEncounterController { channel },
        ))
    }
}

fn validate(
    a: &mut CrecheReviewedArtifact,
    catalog: &MakeCatalog,
    packages: &MakePackageSet,
    bounds: JoiningProductionBounds,
) -> Result<(), String> {
    let checked =
        check_host_configuration(a.configuration.configuration().clone(), catalog, packages)
            .map_err(|e| format!("Crèche artifact configuration: {e:?}"))?;
    if checked.configuration_id() != a.configuration.configuration_id()
        || checked.profile() != a.configuration.profile()
        || a.artifact_bytes == 0
        || a.artifact_bytes > bounds.maximum_artifact_bytes
        || a.image_manifest.len() > bounds.maximum_manifest_bytes
        || a.source_identity.is_empty()
        || a.source_identity != a.image.manifest.source_identity
    {
        return Err("Crèche artifact envelope exceeds its exact reviewed selection".into());
    }
    let (expected, _) = build_host_image(
        checked.profile().clone(),
        catalog,
        packages,
        &a.output,
        &BuildInputs {
            source_identity: a.source_identity.clone(),
            toolchain_available: true,
        },
    )
    .map_err(|e| format!("Crèche artifact IMAGE: {e:?}"))?;
    if expected != a.image {
        return Err("Crèche artifact IMAGE/configuration mismatch".into());
    }
    conduit_host_make::verify_image_binding(&a.image, &a.image_manifest)
        .map_err(|e| format!("Crèche artifact manifest: {e:?}"))?;
    let metadata = a
        .artifact
        .metadata()
        .map_err(|e| format!("Crèche artifact metadata: {e}"))?;
    if !metadata.is_file() || metadata.len() != a.artifact_bytes {
        return Err("Crèche artifact file length/type mismatch".into());
    }
    a.artifact.rewind().map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0; 8192];
    loop {
        let capacity = (a.artifact_bytes - total).min(buffer.len() as u64).max(1) as usize;
        let n = a
            .artifact
            .read(&mut buffer[..capacity])
            .map_err(|e| format!("Crèche artifact read: {e}"))?;
        if n == 0 {
            break;
        }
        if n as u64 > a.artifact_bytes - total {
            return Err("Crèche artifact grew beyond reviewed length".into());
        }
        total += n as u64;
        hash.update(&buffer[..n]);
    }
    a.artifact
        .seek(SeekFrom::Start(0))
        .map_err(|e| e.to_string())?;
    if total != a.artifact_bytes || format!("sha256:{:x}", hash.finalize()) != a.artifact_digest {
        return Err("Crèche artifact differs from owner-reviewed digest".into());
    }
    Ok(())
}
