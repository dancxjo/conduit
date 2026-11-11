//! Initial single-generation file residence, never an IMAGE reconfiguration.
use super::*;
use crate::text_file::SelectedTextFileResidence;
use conduit_host_hosted::CheckedTextFileInitialization;
use conduit_linux_snapshot_store::SnapshotDirectory;

pub struct PreparedImageTextFile {
    pub(super) entry: BaseProviderEntry,
    pub(super) residence: SelectedTextFileResidence,
}
impl PreparedImageTextFile {
    pub fn prepare(
        config: &StdHostConfig,
        image: &HostImage,
        checked: &conduit_host_make::CheckedHostConfiguration,
        parameters: CheckedTextFileInitialization,
    ) -> Result<Self, String> {
        use std::os::{
            fd::AsRawFd,
            unix::fs::{MetadataExt, OpenOptionsExt},
        };
        if parameters != CheckedTextFileInitialization::from_configuration(checked)?
            || parameters.configuration_id != checked.configuration_id()
            || image.manifest.source_configuration_id.as_deref() != Some(checked.configuration_id())
        {
            return Err("immutable Text parameters differ from selected IMAGE".into());
        }
        let mut bases = image
            .manifest
            .base_selections
            .iter()
            .filter(|base| base.kind == conduit_host_hosted::TEXT_FILE_BASE_KIND);
        let base = bases.next().ok_or("IMAGE lacks immutable Text Base")?;
        let implementation = if parameters.write {
            conduit_std_offers::DURABLE_TEXT_SAVE_IMPLEMENTATION
        } else {
            conduit_std_offers::DURABLE_TEXT_LOAD_IMPLEMENTATION
        };
        let budgets = image
            .manifest
            .resource_budgets
            .iter()
            .filter(|budget| budget.class == "data/immutable-generation@1")
            .collect::<Vec<_>>();
        if bases.next().is_some()
            || base.driver != implementation
            || budgets.len() != 1
            || budgets[0].slots != 1
            || budgets[0].bytes != parameters.disk_capacity_bytes
        {
            return Err("immutable Text requires its exact single-generation IMAGE budget".into());
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&parameters.directory)
            .map_err(|e| format!("selected Text directory: {e}"))?;
        // Cooperative custody lasts for this retained directory descriptor. No
        // lock file or other startup filesystem write is introduced.
        // SAFETY: the opened directory descriptor remains owned by this value.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(format!(
                "selected Text directory custody: {}",
                std::io::Error::last_os_error()
            ));
        }
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        let mut capacity = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: a valid owned descriptor and writable statvfs storage.
        if unsafe { libc::fstatvfs(file.as_raw_fd(), capacity.as_mut_ptr()) } != 0 {
            return Err(format!(
                "selected Text capacity: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: successful fstatvfs initialized this value.
        let capacity = unsafe { capacity.assume_init() };
        if u128::from(capacity.f_bavail) * u128::from(capacity.f_frsize)
            < u128::from(parameters.disk_capacity_bytes)
        {
            return Err("selected Text filesystem capacity is insufficient".into());
        }
        let directory = SnapshotDirectory::prepare(file)
            .map_err(|e| format!("selected Text directory preparation: {e}"))?;
        let reference = parameters.reference;
        let content = ResourceContentRequirement {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile.clone(),
            maximum_bytes: u32::try_from(reference.extent.bytes).map_err(|e| e.to_string())?,
            maximum_items: 1,
            retention: ResourceRetention::ExternalDurable,
            sharing: ResourceSharing::SingleWriterPublished,
            access: if parameters.write {
                ResourceAccessMode::WriteCandidatePublish
            } else {
                ResourceAccessMode::ReadPublished
            },
            generation_slots: 1,
            reader_leases: 1,
            publication_slots: u16::from(parameters.write),
            sensitive: false,
        };
        let resource = ResourceOffer {
            pool_id: budgets[0].id.as_str().into(),
            class_id: reference.access_class.clone(),
            capacity_units: 1,
            compute: None,
            content: Some(ResourceContentOffer {
                contract: content.clone(),
                owner_host: config.host_id.clone(),
                owner_boot: config.boot_id.clone(),
                base_id: conduit_std_offers::DURABLE_TEXT_BASE.into(),
                residence_profile: conduit_std_offers::DURABLE_TEXT_RESIDENCE.into(),
            }),
        };
        Ok(Self {
            entry: BaseProviderEntry {
                base_id: base.id.as_str().into(),
                provider_instance_id: format!(
                    "{}/{}/immutable-text",
                    config.host_id.as_str(),
                    config.boot_id.as_str()
                )
                .into(),
                provider_generation: config.offer_generation.0,
                implementation_id: implementation.into(),
                mechanism_family: conduit_host_hosted::TEXT_FILE_BASE_KIND.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![conduit_std_offers::durable_text_offer(content)?],
                resources: vec![resource],
            },
            residence: SelectedTextFileResidence {
                directory,
                reference,
                root_identity: (metadata.dev(), metadata.ino()),
            },
        })
    }
}
impl StdHost {
    pub fn from_selected_timer_and_text_file_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &conduit_host_make::CheckedHostConfiguration,
        parameters: CheckedTextFileInitialization,
    ) -> Result<Self, String> {
        conduit_host_make::verify_configuration_image(checked, image, catalog)?;
        let text_file = PreparedImageTextFile::prepare(&config, image, checked, parameters)?;
        let cpu = super::cpu::prepare(&config, image, checked)?;
        Self::from_selected_initial_bases(
            config,
            image,
            catalog,
            InitialBases {
                text_file: Some(text_file),
                cpu,
                ..InitialBases::default()
            },
        )
    }
}
pub(crate) fn prepare_from_configuration(
    config: &StdHostConfig,
    image: &HostImage,
    checked: &conduit_host_make::CheckedHostConfiguration,
) -> Result<Option<PreparedImageTextFile>, String> {
    if !checked
        .configuration()
        .bases
        .iter()
        .any(|base| base.kind == conduit_host_hosted::TEXT_FILE_BASE_KIND)
    {
        return Ok(None);
    }
    PreparedImageTextFile::prepare(
        config,
        image,
        checked,
        CheckedTextFileInitialization::from_configuration(checked)?,
    )
    .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::{
        build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
        BuildInputs,
    };
    const SOURCE: &str = include_str!("../../profiles/linux-immutable-text.host.conduit");
    fn checked(source: &str) -> conduit_host_make::CheckedHostConfiguration {
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
        )
        .unwrap()
    }
    fn image(checked: &conduit_host_make::CheckedHostConfiguration) -> HostImage {
        build_default_host_image(
            checked.profile().clone(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            &BuildInputs {
                source_identity: "fixture/immutable-text".into(),
                toolchain_available: true,
            },
        )
        .unwrap()
        .0
    }
    #[test]
    fn selected_file_initialization_is_exact_and_reconfiguration_stays_refused() {
        let directory =
            std::env::temp_dir().join(format!("conduit-image-text-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let source = SOURCE.replace(
            "/var/lib/conduit/immutable-text",
            directory.to_str().unwrap(),
        );
        for read in [false, true] {
            let source = if read {
                source
                    .replace("std/file-data-save-text@1", "std/file-data-load-text@1")
                    .replace("data/save/text", "data/load/text")
                    .replace("write-candidate-publish", "read-published")
            } else {
                source.clone()
            };
            let checked = checked(&source);
            let image = image(&checked);
            let parameters = CheckedTextFileInitialization::from_configuration(&checked).unwrap();
            let mut host = StdHost::from_selected_timer_and_text_file_image(
                StdHostConfig {
                    host_id: "fixture/text".into(),
                    boot_id: if read { "boot/read" } else { "boot/write" }.into(),
                    offer_generation: OfferGeneration(1),
                },
                &image,
                &conduit_workspace_make::catalog(),
                &checked,
                parameters.clone(),
            )
            .unwrap();
            assert_eq!(host.advertisement.capabilities.len(), 2);
            assert_eq!(host.base_registry.entries().len(), 2);
            let content = host
                .advertisement
                .resources
                .iter()
                .find_map(|r| r.content.as_ref())
                .unwrap();
            assert_eq!(content.contract.maximum_bytes, 256);
            assert_eq!(content.owner_boot, host.advertisement.boot_id);
            assert_eq!(
                content.contract.access,
                if read {
                    ResourceAccessMode::ReadPublished
                } else {
                    ResourceAccessMode::WriteCandidatePublish
                }
            );
            let competing = PreparedImageTextFile::prepare(
                &StdHostConfig {
                    host_id: "fixture/other".into(),
                    boot_id: "boot/other".into(),
                    offer_generation: OfferGeneration(1),
                },
                &image,
                &checked,
                parameters.clone(),
            );
            assert!(competing.err().unwrap().contains("directory custody"));
            let before = host.advertisement.clone();
            assert!(host
                .transition_text_file_offer(
                    std::fs::File::open(&directory).unwrap(),
                    parameters.reference,
                    read
                )
                .unwrap_err()
                .contains("fresh checked construction"));
            assert_eq!(host.advertisement, before);
        }
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn selected_file_initialization_rejects_wrong_access_and_insufficient_budget() {
        let checked = checked(SOURCE);
        let mut parameters = CheckedTextFileInitialization::from_configuration(&checked).unwrap();
        parameters.disk_capacity_bytes += 1;
        assert!(PreparedImageTextFile::prepare(
            &StdHostConfig {
                host_id: "fixture/text".into(),
                boot_id: "boot/test".into(),
                offer_generation: OfferGeneration(1)
            },
            &image(&checked),
            &checked,
            parameters
        )
        .is_err());
        let changed = self::checked(&SOURCE.replace("write-candidate-publish", "read-published"));
        assert!(CheckedTextFileInitialization::from_configuration(&changed).is_err());
    }
}
