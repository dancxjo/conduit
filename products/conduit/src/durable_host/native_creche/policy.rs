//! Explicit installed-Host grants, separate from authored child Plot meaning.
use conduit::creche_production::{CrecheImageInput, CrecheLinuxBuildGrant, CrecheReviewedArtifact};
use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
use conduit_body::ResidentPlot;
use conduit_body_make::DeploymentDraft;
use conduit_host_make::{BuilderBounds, BuilderSource, ToolchainGrant};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    pub schema: String,
    pub construction: PathBuf,
    pub first_host: String,
    #[serde(default)]
    pub starting_sources: Vec<PathBuf>,
    pub reviewed_images: BTreeMap<String, ReviewedImage>,
    pub source_builds: BTreeMap<String, SourceBuild>,
    pub staging_directory: PathBuf,
    pub save_directory: PathBuf,
    pub maximum_total_bytes: u64,
    pub maximum_millis: u64,
    pub width: u32,
    pub height: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReviewedImage {
    resolved_image: PathBuf,
    target_manifest: PathBuf,
    artifact: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceBuild {
    source: BuilderSource,
    expected_source_identity: String,
    permitted_local_roots: Vec<PathBuf>,
    toolchain: ToolchainGrant,
    limine_archive: PathBuf,
    workspace: PathBuf,
    output_directory: PathBuf,
    bounds: BuilderBounds,
}
impl Policy {
    pub fn load(path: &Path) -> Result<Self, String> {
        let policy: Self = serde_json::from_slice(&small_file(path)?)
            .map_err(|e| format!("native Crèche grant document: {e}"))?;
        if policy.schema != "conduit.install/native-creche-grants@1"
            || policy.starting_sources.len() > 16
            || policy.reviewed_images.len() > conduit_body_make::MAXIMUM_BODY_HOSTS
            || policy.source_builds.len() > conduit_body_make::MAXIMUM_BODY_HOSTS
            || !(1..=86_400_000).contains(&policy.maximum_millis)
            || policy.maximum_total_bytes == 0
            || policy.maximum_total_bytes > 16 * 1024 * 1024 * 1024
            || !(320..=3840).contains(&policy.width)
            || !(240..=2160).contains(&policy.height)
        {
            return Err("native Crèche grant identity or finite bounds refused".into());
        }
        for directory in [&policy.staging_directory, &policy.save_directory] {
            let meta = std::fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
            if !directory.is_absolute() || !meta.is_dir() || meta.file_type().is_symlink() {
                return Err(
                    "native Crèche requires explicitly selected existing directories".into(),
                );
            }
        }
        Ok(policy)
    }
    pub fn draft(&self) -> Result<DeploymentDraft, String> {
        let construction = crate::body_product::load(&self.construction)?;
        let mut choices = Vec::new();
        let documents = if self.starting_sources.is_empty() {
            conduitos::native_workset::candidate_inventory()
                .iter()
                .map(|p| p.source().to_owned())
                .collect::<Vec<_>>()
        } else {
            self.starting_sources
                .iter()
                .map(|path| String::from_utf8(small_file(path)?).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, String>>()?
        };
        for document in documents {
            // Native candidate Source is checked by its owning catalogue. It
            // includes native Kinds absent from the parent's hosted catalogue.
            if let Some(plot) = conduitos::native_workset::candidate_inventory()
                .into_iter()
                .find(|p| p.source() == document)
            {
                let checked = conduitos::native_workset::checked(plot)
                    .map_err(|e| format!("native Source check: {e:?}"))?;
                let identity =
                    ResidentPlot::new(checked.source_document_id, checked.checked_plot_id);
                if choices.len() == conduit_body::MAX_BODY_PLOTS
                    || choices.iter().any(|c: &BirthPlotChoice| c.plot == identity)
                {
                    return Err("starting Plot catalogue duplicate or bound exceeded".into());
                }
                let refusal = match conduitos::native_workset::profile().contains(&identity) {
                    Ok(true) => None,
                    _ => Some(
                        "No installed native starting-Plot consumer for this exact Source".into(),
                    ),
                };
                choices.push(BirthPlotChoice {
                    title: plot.title().into(),
                    search_text: plot.name().into(),
                    plot: identity,
                    refusal,
                    selected: false,
                });
                continue;
            }
            let source = crate::plot_source::parse(&document)?;
            let checked = source.check()?;
            for plot in &checked.plots {
                if choices.len() == conduit_body::MAX_BODY_PLOTS {
                    return Err("starting Plot catalogue bound exceeded".into());
                }
                let identity = ResidentPlot::new(
                    checked.source_document_id.clone(),
                    plot.checked_plot_id.clone(),
                );
                source.expand_identity_for_authoring(
                    &identity.source_document_id,
                    &identity.checked_plot_id,
                )?;
                if choices.iter().any(|c: &BirthPlotChoice| c.plot == identity) {
                    return Err("duplicate starting Plot identity".into());
                }
                choices.push(BirthPlotChoice {
                    title: plot.name.clone(),
                    search_text: plot.name.clone(),
                    plot: identity.clone(),
                    // Current installed inventory is finite eligibility only.
                    // Destination admission still checks its exact image/Host.
                    refusal: match conduitos::native_workset::profile().contains(&identity) {
                        Ok(true) => None,
                        _ => Some(
                            "No installed native starting-Plot consumer for this exact Source"
                                .into(),
                        ),
                    },
                    selected: false,
                });
            }
        }
        let birth = BirthDraft::new(crate::birth_identity::fresh_uuid()?, choices)
            .map_err(|e| format!("birth draft: {e:?}"))?;
        DeploymentDraft::new(birth, construction, self.first_host.clone())
            .map_err(|e| format!("construction: {e:?}"))
    }
}
impl ReviewedImage {
    pub fn open(self) -> Result<CrecheImageInput, String> {
        Ok(CrecheImageInput::ReviewedRelease(Box::new(
            CrecheReviewedArtifact {
                image: serde_json::from_slice(&small_file(&self.resolved_image)?)
                    .map_err(|e| e.to_string())?,
                target_build_manifest: small_file(&self.target_manifest)?,
                file: File::open(self.artifact).map_err(|e| e.to_string())?,
            },
        )))
    }
}
impl SourceBuild {
    pub fn grant(self) -> CrecheImageInput {
        CrecheImageInput::LinuxSource(Box::new(CrecheLinuxBuildGrant {
            source: self.source,
            expected_source_identity: self.expected_source_identity,
            permitted_local_roots: self.permitted_local_roots,
            permitted_network_origins: vec![],
            toolchain: self.toolchain,
            boot_assets: conduit_host_hosted::ReviewedConduitosBootAssets::local_limine_archive(
                self.limine_archive,
            ),
            workspace: self.workspace,
            output_directory: self.output_directory,
            bounds: self.bounds,
        }))
    }
}
pub(super) fn small_file(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = File::open(path).map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > 1024 * 1024 {
        return Err("native Crèche metadata file bound refused".into());
    }
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("native Crèche metadata grew beyond bound".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
