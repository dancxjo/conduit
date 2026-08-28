use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CheckedBodyDescription, SporeJoinMode};
use conduit_host_fabrication::{
    build_host_image, BuildDiagnostic, BuildInputs, FabricationBuildSelection, FabricationCatalog,
    FabricationPackageSet, HostImage, SporeOutputKind,
};

pub const SPORE_MANIFEST_SCHEMA: &str = "conduit.body/spore-manifest@2";
pub const DEPLOYMENT_RECEIPT_SCHEMA: &str = "conduit.body/spore-deployment@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum SporeBinding {
    Prejoined { part_id: String },
    SelfJoining { invitation_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SporeManifest {
    pub schema: String,
    pub spore_id: String,
    pub body_id: String,
    pub binding: SporeBinding,
    pub body_description_id: String,
    pub host_entry_name: String,
    pub host_configuration_id: String,
    pub profile_id: String,
    pub build_id: String,
    pub image_id: String,
    pub target: String,
    pub output: SporeOutputKind,
    pub fabrication: FabricationBuildSelection,
    pub source_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltSpore {
    pub manifest: SporeManifest,
    pub image: HostImage,
    pub image_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentReceipt {
    pub schema: String,
    pub spore_id: String,
    pub image_id: String,
    pub adapter: String,
    pub destination: String,
    pub artifact_digest: String,
    pub disposition: DeploymentDisposition,
    pub does_not_prove: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeploymentDisposition {
    Prepared,
    Deployed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyBuildDiagnostic {
    UnknownHost {
        name: String,
    },
    Fabrication {
        host: String,
        detail: String,
    },
    HostBuild {
        host: String,
        diagnostics: Vec<BuildDiagnostic>,
    },
    Encode {
        detail: String,
    },
    DeploymentMetadataMissing {
        host: String,
    },
    DeploymentAdapterMissing {
        host: String,
    },
}

pub fn build_body_spores(
    body: &CheckedBodyDescription,
    selected_host: Option<&str>,
    source_identity: &str,
    catalog: &FabricationCatalog,
    packages: &FabricationPackageSet,
) -> Result<Vec<BuiltSpore>, Vec<BodyBuildDiagnostic>> {
    if selected_host.is_some_and(|selected| {
        !body
            .hosts()
            .iter()
            .any(|host| host.description.name == selected)
    }) {
        return Err(vec![BodyBuildDiagnostic::UnknownHost {
            name: selected_host.unwrap_or_default().into(),
        }]);
    }
    let mut built = Vec::new();
    let mut diagnostics = Vec::new();
    for host in body
        .hosts()
        .iter()
        .filter(|host| selected_host.is_none_or(|selected| host.description.name == selected))
    {
        let profile = host.configuration.profile().clone();
        let fabrication =
            match packages.derive_build_selection(&profile, &host.description.spore.output) {
                Ok(selection) => selection,
                Err(error) => {
                    diagnostics.push(BodyBuildDiagnostic::Fabrication {
                        host: host.description.name.clone(),
                        detail: format!("{error:?}"),
                    });
                    continue;
                }
            };
        let inputs = BuildInputs {
            source_identity: source_identity.into(),
            toolchain_available: true,
        };
        let (image, image_bytes) = match build_host_image(
            profile,
            catalog,
            packages,
            &host.description.spore.output,
            &inputs,
        ) {
            Ok(value) => value,
            Err(items) => {
                diagnostics.push(BodyBuildDiagnostic::HostBuild {
                    host: host.description.name.clone(),
                    diagnostics: items,
                });
                continue;
            }
        };
        let binding = match host.description.spore.join_mode {
            SporeJoinMode::Prejoined => SporeBinding::Prejoined {
                part_id: host
                    .description
                    .part
                    .clone()
                    .expect("checked prejoined Part"),
            },
            SporeJoinMode::SelfJoining => SporeBinding::SelfJoining {
                invitation_id: host
                    .description
                    .spore
                    .invitation
                    .clone()
                    .expect("checked invitation"),
            },
        };
        let mut manifest = SporeManifest {
            schema: SPORE_MANIFEST_SCHEMA.into(),
            spore_id: String::new(),
            body_id: body.description().body.id.clone(),
            binding,
            body_description_id: body.description_id().into(),
            host_entry_name: host.description.name.clone(),
            host_configuration_id: host.configuration.configuration_id().into(),
            profile_id: image.manifest.profile_id.clone(),
            build_id: image.manifest.build_id.clone(),
            image_id: image.manifest.image_id.clone(),
            target: image.manifest.target.clone(),
            output: host.description.spore.output.clone(),
            fabrication,
            source_identity: source_identity.into(),
        };
        let basis = serde_json::to_vec(&manifest).map_err(|error| {
            vec![BodyBuildDiagnostic::Encode {
                detail: error.to_string(),
            }]
        })?;
        manifest.spore_id = format!("spore:sha256:{:x}", Sha256::digest(&basis));
        built.push(BuiltSpore {
            manifest,
            image,
            image_bytes,
        });
    }
    if diagnostics.is_empty() {
        Ok(built)
    } else {
        Err(diagnostics)
    }
}

pub fn deployment_receipt(
    body: &CheckedBodyDescription,
    spore: &BuiltSpore,
    disposition: DeploymentDisposition,
) -> Result<DeploymentReceipt, BodyBuildDiagnostic> {
    let host = body
        .hosts()
        .iter()
        .find(|host| host.description.name == spore.manifest.host_entry_name)
        .expect("built Spore host remains checked");
    let destination = host
        .description
        .deployment
        .as_ref()
        .ok_or_else(|| BodyBuildDiagnostic::DeploymentMetadataMissing {
            host: host.description.name.clone(),
        })?
        .destination
        .clone();
    let adapter = spore
        .manifest
        .fabrication
        .deployment_adapter
        .clone()
        .ok_or_else(|| BodyBuildDiagnostic::DeploymentAdapterMissing {
            host: host.description.name.clone(),
        })?;
    Ok(DeploymentReceipt {
        schema: DEPLOYMENT_RECEIPT_SCHEMA.into(),
        spore_id: spore.manifest.spore_id.clone(),
        image_id: spore.manifest.image_id.clone(),
        adapter,
        destination,
        artifact_digest: format!("sha256:{:x}", Sha256::digest(&spore.image_bytes)),
        disposition,
        does_not_prove: vec![
            "boot".into(),
            "join".into(),
            "presence".into(),
            "runtime-readiness".into(),
        ],
    })
}
