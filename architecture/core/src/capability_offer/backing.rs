//! Requested owned payload of one portable semantic Kind, never a Back grant.
use super::*;
use crate::metadata_backing::Bytes;
impl Kind {
    /// Reachable owned capacities; excludes inline Self, allocator overhead,
    /// checking/planning scratch and runtime resource possession. Shared nested
    /// schema backing is conservatively charged in full per referring owner by
    /// its existing visitor; this is not exclusive/deduplicated caller custody.
    /// Unsupported descendants or arithmetic overflow return None.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let Kind {
            startup_parameters,
            shorthand,
            kind_id,
            kind_contract_revision,
            inputs,
            outputs,
            configuration,
            semantic_laws,
            limits: _,
        } = self;
        let mut n = Bytes::default();
        n.startup(startup_parameters)?;
        n.shorthand(shorthand)?;
        n.strings(&[&kind_id.0, &kind_contract_revision.0])?;
        n.ports(inputs)?;
        n.ports(outputs)?;
        n.vector(configuration)?;
        for field in configuration {
            n.add(field.retained_backing_bytes()?)?;
        }
        n.laws(semantic_laws)?;
        Some(n.0)
    }
}
