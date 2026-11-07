//! Selected immutable Text generations beneath ordinary data/save and data/load.
//! A directory descriptor and exact Plan binding are supplied by Host preparation;
//! no path, namespace discovery, editor state or automatic fallback is introduced.
use conduit_core::{AuthorityBinding, BoundedResourceRef, PlannedGear};
use conduit_data::DataReference;
use conduit_linux_snapshot_store::{FileSnapshotRefusal, PreparedFileSnapshot, SnapshotDirectory};

#[derive(Debug)]
pub enum TextFileRefusal {
    InvalidBinding,
    WrongReference,
    InvalidText,
    Unavailable,
    File(FileSnapshotRefusal),
}
impl From<FileSnapshotRefusal> for TextFileRefusal {
    fn from(value: FileSnapshotRefusal) -> Self {
        Self::File(value)
    }
}

/// Prepared one-shot publication or exact read. It does not own the document.
/// The Host separately admits disk capacity, current Play and operation work.
pub struct PreparedTextFile<'a> {
    file: PreparedFileSnapshot<'a>,
    reference: Vec<u8>,
    write: bool,
    retired: bool,
}
impl<'a> PreparedTextFile<'a> {
    pub fn prepare(
        directory: &'a SnapshotDirectory,
        placement: &PlannedGear,
        reference: &BoundedResourceRef,
    ) -> Result<Self, TextFileRefusal> {
        let content = placement
            .resources
            .first()
            .and_then(|r| r.content.as_ref())
            .ok_or(TextFileRefusal::InvalidBinding)?;
        if placement.resources.len() != 1
            || placement.resources[0].protected.is_some()
            || placement.resources[0].compute.is_some()
            || content.base_id.as_str() != conduit_std_offers::DURABLE_TEXT_BASE
            || content.residence_profile.as_str() != conduit_std_offers::DURABLE_TEXT_RESIDENCE
        {
            return Err(TextFileRefusal::InvalidBinding);
        }
        let offer = conduit_std_offers::durable_text_offer(content.contract.clone())
            .map_err(|_| TextFileRefusal::InvalidBinding)?;
        if placement.semantic_contract != offer.semantic_contract
            || placement.kind_id != offer.kind_id
            || placement.kind_contract_revision != offer.kind_contract_revision
            || placement.implementation_id != offer.implementation.implementation_id
            || placement.execution_profile_id != offer.implementation.execution_profile_id
            || placement.artifact_id != offer.implementation.artifact_id
            || placement.capability_id != offer.capability_id
            || placement.host_calls != offer.host_calls
            || placement.inputs != offer.inputs
            || placement.outputs != offer.outputs
            || placement.limits != offer.limits
            || !placement.configuration.is_empty()
        {
            return Err(TextFileRefusal::InvalidBinding);
        }
        let data =
            DataReference::new(reference.clone()).map_err(|_| TextFileRefusal::WrongReference)?;
        data.validate_for(&conduit_core::kind_id("value/text"))
            .map_err(|_| TextFileRefusal::WrongReference)?;
        let encoded = data.encode().map_err(|_| TextFileRefusal::WrongReference)?;
        Ok(Self {
            file: PreparedFileSnapshot::prepare(directory, placement, reference)?,
            reference: encoded,
            write: content.contract.access
                == conduit_core::ResourceAccessMode::WriteCandidatePublish,
            retired: false,
        })
    }

    /// The reference becomes an output only after the actual file and directory
    /// publication boundary succeeds. Refusal never acknowledges a Save.
    pub fn publish(
        &mut self,
        authority: &AuthorityBinding,
        text: &[u8],
    ) -> Result<&[u8], TextFileRefusal> {
        if self.retired {
            return Err(TextFileRefusal::Unavailable);
        }
        if !self.write {
            return Err(TextFileRefusal::InvalidBinding);
        }
        core::str::from_utf8(text).map_err(|_| TextFileRefusal::InvalidText)?;
        self.file.publish(authority, text)?;
        Ok(&self.reference)
    }

    pub fn load(
        &mut self,
        authority: &AuthorityBinding,
        reference: &[u8],
    ) -> Result<&[u8], TextFileRefusal> {
        if self.retired {
            return Err(TextFileRefusal::Unavailable);
        }
        if self.write {
            return Err(TextFileRefusal::InvalidBinding);
        }
        if reference != self.reference {
            return Err(TextFileRefusal::WrongReference);
        }
        let text = self.file.read(authority)?;
        core::str::from_utf8(text).map_err(|_| TextFileRefusal::InvalidText)?;
        Ok(text)
    }

    /// Rejoining requires fresh Host preparation and authority; an unavailable
    /// provider never acknowledges another write from its retained binding.
    pub fn retire(&mut self) {
        self.retired = true;
    }
}

pub(crate) struct SelectedTextFileResidence {
    pub(crate) directory: SnapshotDirectory,
    pub(crate) reference: BoundedResourceRef,
    root_identity: (u64, u64),
}

impl crate::StdHost {
    /// Install one explicitly selected file-generation Back. This replaces the
    /// Play-local data offers, so a durable request cannot fall back to RAM.
    pub fn new_with_text_file(
        config: crate::StdHostConfig,
        directory: std::fs::File,
        reference: BoundedResourceRef,
        write: bool,
    ) -> Result<Self, String> {
        let mut host = Self::new_with_config(config);
        host.install_text_file(directory, reference, write, false)?;
        Ok(host)
    }

    /// Select a fresh generation or read grant on the same idle Host, retaining
    /// its Boot, equipment and execution cursors. Replanning is mandatory.
    pub fn transition_text_file_offer(
        &mut self,
        directory: std::fs::File,
        reference: BoundedResourceRef,
        write: bool,
    ) -> Result<(), String> {
        self.install_text_file(directory, reference, write, true)
    }

    fn install_text_file(
        &mut self,
        directory: std::fs::File,
        reference: BoundedResourceRef,
        write: bool,
        advance_offer: bool,
    ) -> Result<(), String> {
        self.kernel_resources.require_reference_reconfiguration()?;
        if !self.kernel_resources.is_idle() {
            return Err("Text file selection requires an idle Host".into());
        }
        use conduit_core::*;
        DataReference::new(reference.clone())
            .and_then(|data| data.validate_for(&kind_id("value/text")))
            .map_err(|error| format!("selected Text reference: {error:?}"))?;
        if reference.extent.bytes > u64::from(conduit_data::MAXIMUM_DATA_TEXT_BYTES) {
            return Err("selected Text exceeds the durable file profile".into());
        }
        use std::os::unix::fs::MetadataExt;
        let metadata = directory
            .metadata()
            .map_err(|error| format!("selected Text root: {error}"))?;
        let root_identity = (metadata.dev(), metadata.ino());
        if let Some(current) = &self.selected_text_file {
            if current.root_identity != root_identity
                || current.reference.identity != reference.identity
            {
                return Err(
                    "Text file transition changed its selected root or document identity".into(),
                );
            }
        }
        let directory = SnapshotDirectory::prepare(directory)
            .map_err(|error| format!("selected Text directory: {error}"))?;
        let mut advertisement = self.advertisement.clone();
        if advance_offer {
            advertisement.offer_generation = OfferGeneration(
                advertisement
                    .offer_generation
                    .0
                    .checked_add(1)
                    .ok_or("Text file offer generation exhausted")?,
            );
        }
        advertisement
            .resources
            .retain(|resource| resource.pool_id.as_str() != "std/selected-file-text");
        let content = ResourceContentRequirement {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile.clone(),
            maximum_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
            maximum_items: 1,
            retention: ResourceRetention::ExternalDurable,
            sharing: ResourceSharing::SingleWriterPublished,
            access: if write {
                ResourceAccessMode::WriteCandidatePublish
            } else {
                ResourceAccessMode::ReadPublished
            },
            generation_slots: 1,
            reader_leases: 1,
            publication_slots: u16::from(write),
            sensitive: false,
        };
        let offer = conduit_std_offers::durable_text_offer(content.clone())?;
        advertisement.capabilities.retain(|offer| {
            ![
                conduit_data::DATA_SAVE_TEXT_KIND,
                conduit_data::DATA_LOAD_TEXT_KIND,
            ]
            .contains(&offer.kind_id.as_str())
        });
        advertisement.capabilities.push(offer);
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        advertisement.resources.push(ResourceOffer {
            pool_id: "std/selected-file-text".into(),
            class_id: reference.access_class.clone(),
            capacity_units: 1,
            compute: None,
            content: Some(ResourceContentOffer {
                contract: content,
                owner_host: advertisement.host_id.clone(),
                owner_boot: advertisement.boot_id.clone(),
                base_id: conduit_std_offers::DURABLE_TEXT_BASE.into(),
                residence_profile: kind_id(conduit_std_offers::DURABLE_TEXT_RESIDENCE),
            }),
        });
        advertisement.resources.sort();
        let ledger = self.kernel_resources.replacement_for(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        self.selected_text_file = Some(SelectedTextFileResidence {
            directory,
            reference,
            root_identity,
        });
        Ok(())
    }
}

impl TextFileRefusal {
    pub(crate) fn detail(&self) -> u16 {
        use conduit_linux_snapshot_store::{FileSnapshotRefusal as F, IoStage};
        match self {
            Self::InvalidBinding => 10,
            Self::WrongReference => 11,
            Self::InvalidText => 12,
            Self::Unavailable => 13,
            Self::File(F::Binding(_)) => 14,
            Self::File(F::InvalidKey | F::InvalidFile) => 15,
            Self::File(F::AlreadyAttempted) => 16,
            Self::File(F::CandidateNameExists) => 17,
            Self::File(F::PublishedNameExists) => 18,
            Self::File(F::Missing) => 19,
            Self::File(F::Io {
                attempt_may_have_published: true,
                ..
            }) => 29,
            Self::File(F::Io { stage, .. }) => {
                20 + match stage {
                    IoStage::CreateCandidate => 0,
                    IoStage::WriteCandidate => 1,
                    IoStage::SyncCandidate => 2,
                    IoStage::PublishLink => 3,
                    IoStage::RemoveCandidate => 4,
                    IoStage::SyncDirectory => 5,
                    IoStage::OpenRead => 6,
                    IoStage::Read => 7,
                }
            }
        }
    }
}
