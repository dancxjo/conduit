//! Explicit local operator selection for immutable ambient implementation assets.
//! Neither a Face image reference nor network control can select these paths.
use super::*;
use std::path::PathBuf;
const FILE: &str = "ambient-policy.json";
const SCHEMA: &str = "conduit.install/ambient-policy@1";
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) enum Role {
    Source,
    Mask,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    schema: String,
    host_id: String,
    configuration_id: String,
    release_bundle_sha256: String,
    body_id: conduit_body::BodyId,
    role: Role,
    manifest: PathBuf,
    manifest_sha256: [u8; 32],
}
impl Selection {
    fn validate(&self, installation: &Installation) -> Result<(), String> {
        let selected = installation
            .selected_host_configuration
            .as_ref()
            .ok_or("ambient policy requires a selected checked IMAGE")?;
        let body = installation
            .body_state
            .as_ref()
            .map(|b| b.body_id.as_str())
            .or_else(|| {
                installation
                    .joined_body_state
                    .as_ref()
                    .map(|b| b.body_id.as_str())
            });
        if self.schema != SCHEMA
            || self.host_id != installation.host_id
            || self.configuration_id != selected.configuration_id()
            || self.release_bundle_sha256 != installation.release_bundle_sha256
            || body != Some(self.body_id.as_str())
            || !self.manifest.is_absolute()
            || self.manifest.as_os_str().len() > 4096
            || self.manifest_sha256 == [0; 32]
            || selected.host_image().is_none()
        {
            return Err(
                "ambient policy differs from actual Host, Body, release or checked IMAGE".into(),
            );
        }
        let checked = selected.checked()?;
        match self.role {
            Role::Source => {
                conduit_workspace_make::CheckedAmbientInitialization::from_configuration(&checked)?;
            }
            Role::Mask => {
                conduit_workspace_make::CheckedNativeMaskInitialization::from_configuration(
                    &checked,
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn source_installation(
        &self,
        config: &conduit_std_host::StdHostConfig,
        checked: &conduit_host_make::CheckedHostConfiguration,
    ) -> Result<conduit::source_check_worker::AmbientImageInstallation, String> {
        use conduit_std_offers::*;
        if self.role != Role::Source {
            return Err("ambient policy did not select Source assets".into());
        }
        let init =
            conduit_workspace_make::CheckedAmbientInitialization::from_configuration(checked)?;
        let (kind, implementation, authority, call) = match init.lifetime {
            conduit_workspace_make::AmbientObservationLifetime::Finite(_) => (
                "ambient/home-observation",
                AMBIENT_SOURCE_IMPLEMENTATION,
                AMBIENT_SOURCE_AUTHORITY,
                AMBIENT_SOURCE_CALL,
            ),
            conduit_workspace_make::AmbientObservationLifetime::Resident => (
                "ambient/resident-observation",
                AMBIENT_RESIDENT_SOURCE_IMPLEMENTATION,
                AMBIENT_RESIDENT_SOURCE_AUTHORITY,
                AMBIENT_RESIDENT_SOURCE_CALL,
            ),
        };
        Ok(conduit::source_check_worker::AmbientImageInstallation {
            grant: conduit_core::AuthorityGrant {
                grant_id: format!("grant/local-ambient/{}", config.boot_id.as_str()).into(),
                contract_id: authority.into(),
                host_call_contract_id: call.into(),
                subject_kind: kind.into(),
                host_id: config.host_id.clone(),
                boot_id: config.boot_id.clone(),
                capability_id: implementation.into(),
            },
            manifest: self.manifest.clone(),
            manifest_sha256: self.manifest_sha256,
        })
    }
    #[cfg(feature = "native-display")]
    pub(super) fn mask_initialization(
        &self,
    ) -> Result<conduit_std_host::NativeMaskImageInitialization, String> {
        if self.role != Role::Mask {
            return Err("ambient policy did not select Mask assets".into());
        }
        let cache = self.manifest.clone();
        let digest = self.manifest_sha256;
        // SAFETY: this exact local stopped-service policy independently selects
        // this Body and immutable cache. It never derives access from received Face.
        unsafe {
            conduit_std_host::NativeMaskImageInitialization::root_authorized(
                self.body_id.clone(),
                conduit_std_host::NATIVE_MASK_IMAGE_PREPARATION_BYTES,
                conduit_std_host::NATIVE_MASK_IMAGE_RETAINED_BYTES,
                move || conduit_std_host::NativeMaskImageData::load_cache(&cache, digest),
            )
        }
    }
}
pub(super) fn retained(
    state: &Path,
    installation: &Installation,
) -> Result<Option<Selection>, String> {
    let path = state.join(FILE);
    match fs::metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("ambient policy metadata:{error}")),
        Ok(_) => (),
    }
    let policy: Selection = serde_json::from_slice(&bounded_read(&path, 16 * 1024)?)
        .map_err(|error| format!("ambient policy:{error}"))?;
    policy.validate(installation)?;
    Ok(Some(policy))
}
pub(super) fn configure(state: &Path, policy: &Path) -> Result<(), String> {
    let _host = owner_lock::acquire(state)?;
    let _body = owner_lock::body(state)?;
    let installation = read_installation(&state.join("installation.json"))?;
    let selected: Selection = serde_json::from_slice(&bounded_read(policy, 16 * 1024)?)
        .map_err(|error| format!("ambient policy:{error}"))?;
    selected.validate(&installation)?;
    // Validate immutable bytes before retaining the operator selection. Actual
    // construction repeats this read under its original IMAGE reservation.
    let _deck = conduit_std_host::ambient_wallpaper::cache::load_deck(
        &selected.manifest,
        selected.manifest_sha256,
    )?;
    write_json_atomic(&state.join(FILE), &selected)?;
    println!("retained ambient implementation assets for the next Boot");
    Ok(())
}
