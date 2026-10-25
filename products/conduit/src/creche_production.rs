//! Actual image manufacture after canonical encounter review. Run this blocking
//! owner on an admitted worker, never inside the kernel's nonblocking poll.
//! One retained preparation survives per-Host failures and retries; no retry
//! repeats birth. Supplied handles/paths are explicit Host grants, not UI text.
use crate::creche_encounter::CrecheReviewedHandoff;
use conduit_body::BodyBiographyEvidence;
use conduit_body_make::{
    bind_prepared_birth_iso, DeploymentImageSource, PreparedBirthArtifactReceipt,
    PreparedBirthArtifactRefusal, PreparedDeployment,
};
use conduit_core::SignId;
use conduit_host_make::{BuilderReceipt, HostImage};
use std::{
    fs::File,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

mod save;
pub use save::{CrecheSavedReceipt, CrecheSaveRefusal};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::CrecheLinuxBuildGrant;

/// A reviewed release is admitted as exact resolved machinery, target build
/// receipt and actual readable artifact. No pathname or generic JSON is media.
pub struct CrecheReviewedArtifact {
    pub image: HostImage,
    pub target_build_manifest: Vec<u8>,
    pub file: File,
}
pub enum CrecheImageInput {
    ReviewedRelease(Box<CrecheReviewedArtifact>),
    #[cfg(target_os = "linux")]
    LinuxSource(Box<CrecheLinuxBuildGrant>),
}

#[derive(Debug)]
pub enum CrecheProductionRefusal {
    Review(String),
    UnknownOrCompletedHost,
    WrongImageSource,
    InvalidBound,
    Cancelled,
    Make(conduit_host_make::BuilderRefusal),
    Artifact(PreparedBirthArtifactRefusal),
    Io(std::io::Error),
}

/// Private construction ensures successful products come from actual byte
/// binding and readback, rather than externally populated receipt fields.
pub struct CrecheProducedArtifact {
    receipt: PreparedBirthArtifactReceipt,
    builder: Option<BuilderReceipt>,
    file: File,
}
impl CrecheProducedArtifact {
    pub fn receipt(&self) -> &PreparedBirthArtifactReceipt {
        &self.receipt
    }
    pub fn builder_receipt(&self) -> Option<&BuilderReceipt> {
        self.builder.as_ref()
    }
}

/// All selected products, re-read after the final Host finished. Construction
/// is private: this value cannot be made from a claimed successful receipt.
pub struct CrecheProducedImages {
    job: CrecheImageProduction,
}
impl CrecheProducedImages {
    pub(crate) fn revalidate(&mut self) -> Result<(), CrecheProductionRefusal> {
        self.job.verify_finished()
    }
    pub fn biography(&self) -> &BodyBiographyEvidence {
        self.job.biography()
    }
    pub fn artifacts(&self) -> &[CrecheProducedArtifact] {
        self.job.artifacts()
    }
    pub fn request(&self) -> conduit_body_make::deployment_runtime::CrecheRequest {
        self.job.request()
    }
    pub fn context(&self) -> &conduit_std_host::hosted_creche::CrecheExecutionContext {
        self.job.context()
    }
    /// Exact retained canonical evidence. Media is prepared; carrier deployment,
    /// destination boot and ordinary membership have not been observed.
    pub fn evidence(&self, maximum_bytes: u64) -> Result<Vec<u8>, CrecheProductionRefusal> {
        #[derive(serde::Serialize)]
        struct Evidence<'a> {
            schema: &'static str,
            request: [u8; 32],
            body_id: &'a str,
            artifacts: Vec<&'a PreparedBirthArtifactReceipt>,
            artifact_prepared: bool,
            carrier_realized: bool,
            boot_observed: bool,
            join_observed: bool,
        }
        let bytes = serde_json::to_vec(&Evidence {
            schema: "conduit.creche/prepared-image-evidence@1",
            request: self.request().0,
            body_id: self.biography().body_id.as_str(),
            artifacts: self.artifacts().iter().map(|a| a.receipt()).collect(),
            artifact_prepared: true,
            carrier_realized: false,
            boot_observed: false,
            join_observed: false,
        })
        .map_err(|e| CrecheProductionRefusal::Review(e.to_string()))?;
        if bytes.len() as u64 > maximum_bytes {
            return Err(CrecheProductionRefusal::InvalidBound);
        }
        Ok(bytes)
    }
}

pub struct CrecheImageProduction {
    prepared: PreparedDeployment,
    request: conduit_body_make::deployment_runtime::CrecheRequest,
    context: conduit_std_host::hosted_creche::CrecheExecutionContext,
    artifacts: Vec<CrecheProducedArtifact>,
    maximum_total_bytes: u64,
    produced_bytes: u64,
    cancelled: Arc<AtomicBool>,
}
impl CrecheImageProduction {
    /// The installed operation owner supplies an attributable birth occurrence
    /// after taking the single exact review. No filesystem effects occur here.
    pub fn prepare(
        handoff: CrecheReviewedHandoff,
        birth_sequence: u64,
        birth_sign: SignId,
        maximum_total_bytes: u64,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, CrecheProductionRefusal> {
        if maximum_total_bytes == 0 {
            return Err(CrecheProductionRefusal::InvalidBound);
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(CrecheProductionRefusal::Cancelled);
        }
        let prepared = handoff
            .draft
            .prepare(
                handoff.review,
                birth_sequence,
                birth_sign,
                &conduit_workspace_make::catalog(),
                &conduit_workspace_make::package_set(),
            )
            .map_err(|e| CrecheProductionRefusal::Review(format!("{e:?}")))?;
        Ok(Self {
            prepared,
            request: handoff.request,
            context: handoff.context,
            artifacts: Vec::new(),
            maximum_total_bytes,
            produced_bytes: 0,
            cancelled,
        })
    }
    pub fn biography(&self) -> &BodyBiographyEvidence {
        self.prepared.birth().evidence()
    }
    pub fn selected_hosts(&self) -> &[String] {
        self.prepared.selected_hosts()
    }
    pub fn artifacts(&self) -> &[CrecheProducedArtifact] {
        &self.artifacts
    }
    pub fn is_complete(&self) -> bool {
        self.artifacts.len() == self.selected_hosts().len()
    }
    pub fn request(&self) -> conduit_body_make::deployment_runtime::CrecheRequest {
        self.request
    }
    pub fn context(&self) -> &conduit_std_host::hosted_creche::CrecheExecutionContext {
        &self.context
    }

    /// Recheck every retained file after the complete batch, including changes
    /// made while later Hosts were building. On refusal, keep this job and its
    /// one recorded birth in the returned error rather than silently rebirthing.
    pub fn finish(mut self) -> Result<CrecheProducedImages, (CrecheProductionRefusal, Box<Self>)> {
        let result = self.verify_finished();
        match result {
            Ok(()) => Ok(CrecheProducedImages { job: self }),
            Err(error) => Err((error, Box::new(self))),
        }
    }

    /// Withdraw a changed/failed product without changing the retained birth.
    /// The returned file remains owner-held evidence; replacement uses a new
    /// empty output through the ordinary checked produce_host operation.
    pub fn withdraw_product(
        &mut self,
        host_entry: &str,
    ) -> Result<CrecheProducedArtifact, CrecheProductionRefusal> {
        let index = self
            .artifacts
            .iter()
            .position(|a| a.receipt.host_entry == host_entry)
            .ok_or(CrecheProductionRefusal::UnknownOrCompletedHost)?;
        let artifact = self.artifacts.remove(index);
        self.produced_bytes -= artifact.receipt.artifact_bytes;
        Ok(artifact)
    }

    fn verify_finished(&mut self) -> Result<(), CrecheProductionRefusal> {
        use sha2::{Digest, Sha256};
        use std::io::{Read, Seek, SeekFrom};
        use CrecheProductionRefusal as E;
        if !self.is_complete() {
            return Err(E::UnknownOrCompletedHost);
        }
        let mut buffer = [0; 64 * 1024];
        for artifact in &mut self.artifacts {
            if artifact.file.metadata().map_err(E::Io)?.len() != artifact.receipt.artifact_bytes {
                return Err(E::Artifact(PreparedBirthArtifactRefusal::InvalidManifest));
            }
            artifact.file.seek(SeekFrom::Start(0)).map_err(E::Io)?;
            let mut hash = Sha256::new();
            let mut bytes = 0_u64;
            loop {
                if self.cancelled.load(Ordering::Acquire) {
                    return Err(E::Cancelled);
                }
                let n = artifact.file.read(&mut buffer).map_err(E::Io)?;
                if n == 0 {
                    break;
                }
                bytes = bytes.checked_add(n as u64).ok_or(E::InvalidBound)?;
                if bytes > artifact.receipt.artifact_bytes {
                    return Err(E::InvalidBound);
                }
                hash.update(&buffer[..n]);
            }
            if bytes != artifact.receipt.artifact_bytes
                || format!("sha256:{:x}", hash.finalize()) != artifact.receipt.artifact_sha256
            {
                return Err(E::Artifact(PreparedBirthArtifactRefusal::InvalidManifest));
            }
        }
        if self.cancelled.load(Ordering::Acquire) {
            return Err(E::Cancelled);
        }
        Ok(())
    }

    /// Produce one selected Host into an explicitly admitted empty read/write
    /// regular file. Failures preserve this job's birth and previous products;
    /// caller-owned partial files/workspaces are never deleted. Retry with a new
    /// empty output handle and fresh build workspace, retaining this same job.
    pub fn produce_host(
        &mut self,
        host_entry: &str,
        input: CrecheImageInput,
        mut output: File,
    ) -> Result<&CrecheProducedArtifact, CrecheProductionRefusal> {
        use CrecheProductionRefusal as E;
        if self.cancelled.load(Ordering::Acquire) {
            return Err(E::Cancelled);
        }
        let host = self
            .prepared
            .construction()
            .hosts()
            .iter()
            .find(|h| h.description.name == host_entry)
            .filter(|_| {
                self.prepared
                    .selected_hosts()
                    .iter()
                    .any(|name| name == host_entry)
            })
            .filter(|_| {
                !self
                    .artifacts
                    .iter()
                    .any(|a| a.receipt.host_entry == host_entry)
            })
            .ok_or(E::UnknownOrCompletedHost)?;
        let remaining = self
            .maximum_total_bytes
            .checked_sub(self.produced_bytes)
            .filter(|remaining| *remaining > 0)
            .ok_or(E::InvalidBound)?;
        // Refuse output reuse before expensive source work. The binder also
        // independently validates handle modes, regularity and actual bytes.
        let metadata = output.metadata().map_err(E::Io)?;
        if !metadata.is_file() || metadata.len() != 0 {
            return Err(E::InvalidBound);
        }
        let (mut artifact, builder) = match (self.prepared.image_source(), input) {
            (
                DeploymentImageSource::ReviewedRelease,
                CrecheImageInput::ReviewedRelease(artifact),
            ) => (*artifact, None),
            #[cfg(target_os = "linux")]
            (DeploymentImageSource::SourceBuild, CrecheImageInput::LinuxSource(grant)) => {
                let (artifact, receipt) = linux::build(
                    &self.context,
                    &host.configuration,
                    &host.description.spore.output,
                    *grant,
                    remaining,
                    self.cancelled.clone(),
                )?;
                (artifact, Some(receipt))
            }
            _ => return Err(E::WrongImageSource),
        };
        let media = self
            .prepared
            .birth_media_for_image(host_entry, &artifact.image)
            .map_err(E::Artifact)?;
        let cancelled = self.cancelled.clone();
        let receipt = bind_prepared_birth_iso(
            &mut artifact.file,
            &mut output,
            &artifact.image,
            &media,
            &artifact.target_build_manifest,
            remaining,
            &mut || {
                if cancelled.load(Ordering::Acquire) {
                    Err(PreparedBirthArtifactRefusal::Cancelled)
                } else {
                    Ok(())
                }
            },
        )
        .map_err(E::Artifact)?;
        self.produced_bytes = self
            .produced_bytes
            .checked_add(receipt.artifact_bytes)
            .ok_or(E::InvalidBound)?;
        self.artifacts.push(CrecheProducedArtifact {
            receipt,
            builder,
            file: output,
        });
        Ok(self
            .artifacts
            .last()
            .expect("just retained verified artifact"))
    }
}

#[cfg(test)]
pub(crate) mod tests;
