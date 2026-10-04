//! Retain one exact packaged entry through native preparation.
use super::*;
use alloc::format;
use conduit_core::{ArtifactId, BaseImplementationId, HostAdvertisement};
use conduit_planner::{PlacementChoices, PlanningOptions};
use conduit_plot::ExpandedAuthoringPlot;
use sha2::{Digest, Sha256};

pub struct PreparedProtocolEntry {
    source: PreparedProtocolSource,
    expanded: ExpandedAuthoringPlot,
    artifact: ArtifactId,
    package_digest: [u8; 32],
}

impl PreparedProtocolEntry {
    /// Check and retain inert code. The digest identifies exact package bytes;
    /// it establishes neither review nor native resource authority.
    pub fn prepare(bytes: &[u8], entry: &str) -> Result<Self, ProtocolSourceRefusal> {
        if entry.is_empty() || entry.len() > MAXIMUM_SOURCE_BYTES {
            return Err(ProtocolSourceRefusal::Bounds);
        }
        let package = ProtocolSourcePackage::decode(bytes)?;
        let source = PreparedProtocolSource::prepare(package)?;
        let expanded = source.expand(entry)?;
        let digest = Sha256::digest(bytes);
        Ok(Self {
            source,
            expanded,
            artifact: ArtifactId::from(format!("conduitos/protocol-source/sha256/{digest:x}")),
            package_digest: digest.into(),
        })
    }

    pub fn package_digest(&self) -> &[u8; 32] {
        &self.package_digest
    }

    pub fn artifact_id(&self) -> &ArtifactId {
        &self.artifact
    }

    pub fn expanded(&self) -> &ExpandedAuthoringPlot {
        &self.expanded
    }

    pub fn resident(&self) -> conduit_body::ResidentPlot {
        conduit_body::ResidentPlot::new(
            self.expanded.expanded.source_document_id.clone(),
            self.expanded.expanded.checked_plot_id.clone(),
        )
    }

    pub fn publish_pure_backs(
        &self,
        host: &mut HostAdvertisement,
    ) -> Result<(), ProtocolSourceRefusal> {
        self.source.publish_pure_backs(&self.expanded, host)
    }

    pub fn placements(
        &self,
        hosts: &[HostAdvertisement],
    ) -> Result<PlacementChoices, ProtocolSourceRefusal> {
        conduit_planner::default_expanded_placements(&self.expanded.expanded, hosts)
            .map_err(ProtocolSourceRefusal::Plan)
    }

    /// Consume the retained expansion and operations with selected native truth.
    pub fn plan(
        self,
        hosts: &[HostAdvertisement],
        placements: &PlacementChoices,
        bases: &[BaseImplementationId],
        options: PlanningOptions<'_>,
    ) -> Result<PreparedProtocolArtifact, ProtocolSourceRefusal> {
        self.source.plan_artifact(
            &self.expanded,
            self.artifact,
            hosts,
            placements,
            bases,
            options,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes() -> Vec<u8> {
        serde_json::to_vec(&ProtocolSourcePackage {
            schema: PACKAGE_SCHEMA.into(),
            source: "plot identity (\n >> input: Boolean...|\n output: Boolean...| >>\n) = (.)"
                .into(),
            specializations: vec![],
        })
        .unwrap()
    }

    #[test]
    fn exact_package_bytes_remain_distinct_from_resident_meaning() {
        let bytes = bytes();
        let first = PreparedProtocolEntry::prepare(&bytes, "identity").unwrap();
        let mut repackaged = bytes.clone();
        repackaged.push(b'\n');
        let second = PreparedProtocolEntry::prepare(&repackaged, "identity").unwrap();
        assert_eq!(first.resident(), second.resident());
        assert_ne!(first.artifact_id(), second.artifact_id());
        assert_eq!(
            first.package_digest().as_slice(),
            Sha256::digest(&bytes).as_slice()
        );
        assert_ne!(first.package_digest(), second.package_digest());
    }

    #[test]
    fn absent_entry_and_malformed_package_refuse_before_native_planning() {
        for entry in [String::new(), "x".repeat(MAXIMUM_SOURCE_BYTES + 1)] {
            assert!(matches!(
                PreparedProtocolEntry::prepare(&bytes(), &entry),
                Err(ProtocolSourceRefusal::Bounds)
            ));
        }
        assert!(matches!(
            PreparedProtocolEntry::prepare(&bytes(), "absent"),
            Err(ProtocolSourceRefusal::Expansion(_))
        ));
        assert!(matches!(
            PreparedProtocolEntry::prepare(b"invalid", "identity"),
            Err(ProtocolSourceRefusal::Encoding)
        ));
    }
}
