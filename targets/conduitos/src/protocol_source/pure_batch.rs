//! One privately owned checked package for a finite set of pure entries.
use super::*;
use alloc::format;
use conduit_core::{ArtifactId, HostAdvertisement, PortId};
use conduit_planner::PlacementChoices;
use conduit_plot::ExpandedAuthoringPlot;
use sha2::{Digest, Sha256};

pub const MAXIMUM_PURE_ENTRIES: usize = 16;

/// Checks the exact package once and retains every requested expansion. This
/// owner admits only pure packages, so entries cannot share mutable protocol
/// operation state. It establishes no native resource or execution authority.
pub struct PreparedPureProtocolBatch {
    source: PreparedProtocolSource,
    expanded: Vec<ExpandedAuthoringPlot>,
    artifact: ArtifactId,
    digest: [u8; 32],
}

/// Borrowing keeps the complete checked Source alive with each exact expansion.
pub struct PreparedPureProtocolEntry<'a> {
    batch: &'a PreparedPureProtocolBatch,
    expanded: &'a ExpandedAuthoringPlot,
}

impl PreparedPureProtocolBatch {
    pub fn prepare(bytes: &[u8], entries: &[&str]) -> Result<Self, ProtocolSourceRefusal> {
        if entries.is_empty()
            || entries.len() > MAXIMUM_PURE_ENTRIES
            || entries
                .iter()
                .any(|entry| entry.is_empty() || entry.len() > MAXIMUM_SOURCE_BYTES)
        {
            return Err(ProtocolSourceRefusal::Bounds);
        }
        let package = ProtocolSourcePackage::decode(bytes)?;
        if !package.specializations.is_empty() {
            return Err(ProtocolSourceRefusal::Specialization("pure entry batch"));
        }
        let source = PreparedProtocolSource::prepare(package)?;
        let mut expanded = Vec::with_capacity(entries.len());
        for entry in entries {
            expanded.push(source.expand(entry)?);
        }
        let digest = Sha256::digest(bytes);
        Ok(Self {
            source,
            expanded,
            artifact: ArtifactId::from(format!("conduitos/protocol-source/sha256/{digest:x}")),
            digest: digest.into(),
        })
    }

    pub fn entry(&self, index: usize) -> Option<PreparedPureProtocolEntry<'_>> {
        self.expanded
            .get(index)
            .map(|expanded| PreparedPureProtocolEntry {
                batch: self,
                expanded,
            })
    }
}

impl PreparedPureProtocolEntry<'_> {
    pub fn package_digest(&self) -> &[u8; 32] {
        &self.batch.digest
    }
    pub fn artifact_id(&self) -> &ArtifactId {
        &self.batch.artifact
    }
    pub fn expanded(&self) -> &ExpandedAuthoringPlot {
        self.expanded
    }
    pub fn resident(&self) -> conduit_body::ResidentPlot {
        conduit_body::ResidentPlot::new(
            self.expanded.expanded.source_document_id.clone(),
            self.expanded.expanded.checked_plot_id.clone(),
        )
    }
    pub fn input_schema(&self, port: &PortId) -> Option<StructuredInfoType> {
        let descriptor = self
            .expanded
            .front
            .inputs()
            .iter()
            .find(|p| &p.port_id == port)?;
        self.batch
            .source
            .checked
            .structured_type(&descriptor.value_kind)
            .cloned()
            .or_else(|| StructuredInfoType::leaf(descriptor.value_kind.clone()).ok())
    }
    pub fn output_schema(&self, port: &PortId) -> Option<StructuredInfoType> {
        let descriptor = self
            .expanded
            .front
            .outputs()
            .iter()
            .find(|p| &p.port_id == port)?;
        self.batch
            .source
            .checked
            .structured_type(&descriptor.value_kind)
            .cloned()
            .or_else(|| StructuredInfoType::leaf(descriptor.value_kind.clone()).ok())
    }
    pub fn publish_pure_backs(
        &self,
        host: &mut HostAdvertisement,
    ) -> Result<(), ProtocolSourceRefusal> {
        self.batch
            .source
            .publish_retained_pure_backs(self.expanded, host)
    }
    pub fn placements(
        &self,
        hosts: &[HostAdvertisement],
    ) -> Result<PlacementChoices, ProtocolSourceRefusal> {
        conduit_planner::default_expanded_placements(&self.expanded.expanded, hosts)
            .map_err(ProtocolSourceRefusal::Plan)
    }
    pub fn queue_limits(
        &self,
        hosts: &[HostAdvertisement],
        placements: &PlacementChoices,
    ) -> Result<ProtocolQueueLimits, ProtocolSourceRefusal> {
        self.batch
            .source
            .retained_queue_limits(self.expanded, hosts, placements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{BOOL_INFO_ID, OfferGeneration, kind_id};

    const IDENTITY: &str = "plot alpha (\n >> input: Boolean...|\n output: Boolean...| >>\n) = (.)\nplot beta (\n >> input: Boolean...|\n output: Boolean...| >>\n) = (.)";

    fn package(source: &str) -> ProtocolSourcePackage {
        ProtocolSourcePackage {
            schema: PACKAGE_SCHEMA.into(),
            source: source.into(),
            specializations: vec![],
        }
    }
    fn host() -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: "fixture/pure-batch".into(),
            boot_id: "fixture/pure-batch/boot".into(),
            offer_generation: OfferGeneration(1),
            profile: "fixture/pure-batch".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        }
    }

    #[test]
    fn every_batch_entry_preserves_complete_single_entry_preparation() {
        let bytes = serde_json::to_vec(&package(IDENTITY)).unwrap();
        let names = ["alpha", "beta"];
        let batch = PreparedPureProtocolBatch::prepare(&bytes, &names).unwrap();
        for (index, name) in names.iter().enumerate() {
            let old = PreparedProtocolEntry::prepare(&bytes, name).unwrap();
            let entry = batch.entry(index).unwrap();
            assert_eq!(entry.package_digest(), old.package_digest());
            assert_eq!(entry.artifact_id(), old.artifact_id());
            assert_eq!(entry.expanded(), old.expanded());
            assert_eq!(entry.resident(), old.resident());
            for port in entry.expanded().front.inputs() {
                assert_eq!(
                    entry.input_schema(&port.port_id),
                    old.input_schema(&port.port_id)
                );
            }
            for port in entry.expanded().front.outputs() {
                assert_eq!(
                    entry.output_schema(&port.port_id),
                    old.output_schema(&port.port_id)
                );
            }
            let mut expected = host();
            let mut actual = host();
            old.publish_pure_backs(&mut expected).unwrap();
            entry.publish_pure_backs(&mut actual).unwrap();
            assert_eq!(actual, expected);
            let old_choices = old.placements(&[expected.clone()]).unwrap();
            let new_choices = entry.placements(&[actual.clone()]).unwrap();
            assert_eq!(new_choices, old_choices);
            let old_limits = old.queue_limits(&[expected], &old_choices).unwrap();
            let new_limits = entry.queue_limits(&[actual], &new_choices).unwrap();
            assert_eq!(new_limits.connections, old_limits.connections);
            assert_eq!(new_limits.boundaries, old_limits.boundaries);
        }
        assert!(batch.entry(names.len()).is_none());
    }

    #[test]
    fn bounds_and_mutable_specializations_refuse_before_batch_publication() {
        let bytes = serde_json::to_vec(&package(IDENTITY)).unwrap();
        for names in [vec![], vec![""], vec!["alpha"; MAXIMUM_PURE_ENTRIES + 1]] {
            assert!(matches!(
                PreparedPureProtocolBatch::prepare(&bytes, &names),
                Err(ProtocolSourceRefusal::Bounds)
            ));
        }
        let oversized = "x".repeat(MAXIMUM_SOURCE_BYTES + 1);
        assert!(matches!(
            PreparedPureProtocolBatch::prepare(&bytes, &[&oversized]),
            Err(ProtocolSourceRefusal::Bounds)
        ));
        for source in ["", oversized.as_str()] {
            let encoded = serde_json::to_vec(&package(source)).unwrap();
            assert!(matches!(
                PreparedPureProtocolBatch::prepare(&encoded, &["alpha"]),
                Err(ProtocolSourceRefusal::Bounds)
            ));
        }
        assert!(PreparedPureProtocolBatch::prepare(&bytes, &["alpha", "missing"]).is_err());
        let mut specialized = package(IDENTITY);
        specialized
            .specializations
            .push(ProtocolSpecialization::SeededFlow {
                value: ProtocolValue {
                    schema: StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
                    contract: CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap(),
                },
            });
        let encoded = serde_json::to_vec(&specialized).unwrap();
        assert!(matches!(
            PreparedPureProtocolBatch::prepare(&encoded, &["alpha"]),
            Err(ProtocolSourceRefusal::Specialization("pure entry batch"))
        ));
    }

    #[test]
    fn native_laws_and_requested_expansions_preserve_single_entry_checks() {
        let law = format!("type Broken = {{\n value: U64\n where .value\n}}\n{IDENTITY}");
        let bytes = serde_json::to_vec(&package(&law)).unwrap();
        assert!(matches!(
            PreparedPureProtocolBatch::prepare(&bytes, &["alpha"]),
            Err(ProtocolSourceRefusal::Source(_))
        ));
        let plot = format!(
            "{IDENTITY}\nplot broken (\n >> input: Boolean...|\n output: Text...| >>\n) = (.)"
        );
        let bytes = serde_json::to_vec(&package(&plot)).unwrap();
        // Source declaration checks and selected expansion checks have distinct scope.
        assert!(PreparedProtocolEntry::prepare(&bytes, "alpha").is_ok());
        assert!(PreparedPureProtocolBatch::prepare(&bytes, &["alpha"]).is_ok());
        assert!(PreparedProtocolEntry::prepare(&bytes, "broken").is_err());
        assert!(PreparedPureProtocolBatch::prepare(&bytes, &["alpha", "broken"]).is_err());
    }
}
