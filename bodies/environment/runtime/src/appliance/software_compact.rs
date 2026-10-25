//! Make descriptions stay borrowed from immutable material. Only initialized
//! evaluators and exact selection fingerprints occupy mutable target storage.
#![cfg_attr(not(feature = "display-output"), allow(dead_code))]
use super::software::{
    initialize_portable, ready_operation, PreparedSoftwareOperation, SoftwareRefusal,
};
use alloc::vec::Vec;
use conduit_core as c;

struct Operation<'a> {
    node: [u8; 48],
    key: c::AssignedIdentity,
    configuration: &'a str,
    calls: Vec<c::HostCallRequirement>,
    offer: &'a [u8],
    prepared: Option<PreparedSoftwareOperation>,
}
pub struct CompactOutputSoftware<'a> {
    host: c::HostId,
    boot: c::BootId,
    source: c::PlotIdentity,
    generation: Option<c::OfferGeneration>,
    operations: Vec<Operation<'a>>,
}
impl<'a> CompactOutputSoftware<'a> {
    /// Initialize checked computations sequentially and discard their decoded
    /// construction descriptions immediately. No current Plan or native owner
    /// is created; linked offer bytes cannot establish readiness by themselves.
    ///
    /// # Safety
    /// Root authenticates the complete Make material, including each offer's
    /// exact agreement with its checked gear, independently of received input.
    pub unsafe fn prepare_make(
        source: c::PlotIdentity,
        material: impl IntoIterator<
            Item = Result<(conduit_plot::CheckedGear, &'a str, &'a [u8]), SoftwareRefusal>,
        >,
        host: c::HostId,
        boot: c::BootId,
    ) -> Result<Self, SoftwareRefusal> {
        let mut operations = Vec::with_capacity(3);
        for entry in material {
            if operations.len() == 3 {
                return Err(SoftwareRefusal::Count);
            }
            let (gear, configuration, offer) = entry?;
            let initialized = initialize_portable(&gear)?;
            let operation = ready_operation(&gear, initialized)?;
            let [config] = operation.configuration.as_slice() else {
                return Err(SoftwareRefusal::Selection);
            };
            if !matches!(config.key.as_str(), "program" | "selector")
                || !matches!(&config.value, c::ConfigurationValue::Text(text) if text == configuration)
                || offer.is_empty()
            {
                return Err(SoftwareRefusal::Selection);
            }
            let mut node = [0; 48];
            for (i, text) in [
                operation.offer.kind_id.as_str(),
                operation.offer.implementation.implementation_id.as_str(),
                operation.offer.implementation.artifact_id.as_str(),
            ]
            .into_iter()
            .enumerate()
            {
                node[i * 16..(i + 1) * 16].copy_from_slice(&c::AssignedIdentity::from_text(text).0);
            }
            if operations
                .iter()
                .any(|previous: &Operation<'_>| previous.node == node)
            {
                return Err(SoftwareRefusal::Selection);
            }
            operations.push(Operation {
                node,
                key: c::AssignedIdentity::from_text(&config.key),
                configuration,
                calls: operation.offer.host_calls,
                offer,
                prepared: operation.prepared,
            });
        }
        if operations.len() != 3 {
            return Err(SoftwareRefusal::Count);
        }
        Ok(Self {
            host,
            boot,
            source,
            generation: None,
            operations,
        })
    }
    pub fn current_offers(
        &mut self,
        host: &c::HostId,
        boot: &c::BootId,
        generation: c::OfferGeneration,
    ) -> Result<CompactOutputOffers<'_, 'a>, SoftwareRefusal> {
        if host != &self.host
            || boot != &self.boot
            || generation.0 == 0
            || self.generation.is_some_and(|old| generation.0 < old.0)
        {
            return Err(SoftwareRefusal::Advertisement);
        }
        self.generation = Some(generation);
        Ok(CompactOutputOffers { owner: self })
    }
    pub(crate) fn take_assigned(
        &mut self,
        identity: c::AssignedPlanView,
        generation: c::OfferGeneration,
        source: &c::PlotIdentity,
        records: &[c::AssignedRecord<'_>],
        write: conduit_kernel::NodeId,
    ) -> Result<[Option<PreparedSoftwareOperation>; 4], SoftwareRefusal> {
        use SoftwareRefusal as E;
        if identity.host != c::AssignedIdentity::from_text(self.host.as_str())
            || identity.boot != c::AssignedIdentity::from_text(self.boot.as_str())
            || generation.0 == 0
            || self.generation != Some(generation)
            || source != &self.source
            || write.0 >= 4
        {
            return Err(E::Selection);
        }
        let mut indices = [None; 4];
        let mut seen = [false; 4];
        for record in records.iter().filter(|r| r.tag == c::ASSIGNED_NODE) {
            let data = record.payload;
            if data.len() != 52 {
                return Err(E::Selection);
            }
            let ordinal = u16_at(data, 0)?;
            let present = seen.get_mut(usize::from(ordinal)).ok_or(E::Selection)?;
            if *present {
                return Err(E::Selection);
            }
            *present = true;
            if ordinal == write.0 {
                continue;
            }
            let index = self
                .operations
                .iter()
                .position(|op| data[4..52] == op.node)
                .ok_or(E::Selection)?;
            if indices.contains(&Some(index)) {
                return Err(E::Selection);
            }
            let op = &self.operations[index];
            if op.prepared.is_none() {
                return Err(E::AlreadySelected);
            }
            let mut configurations = records.iter().filter(|r| {
                r.tag == c::ASSIGNED_CONFIGURATION
                    && r.payload.get(..2) == Some(&ordinal.to_le_bytes())
            });
            let data = configurations.next().ok_or(E::Selection)?.payload;
            if configurations.next().is_some()
                || data.len() < 19
                || data[2..18] != op.key.0
                || data[18] != 3
                || &data[19..] != op.configuration.as_bytes()
            {
                return Err(E::Selection);
            }
            let mut count = 0;
            for record in records.iter().filter(|r| {
                r.tag == c::ASSIGNED_HOST_CALL && r.payload.get(..2) == Some(&ordinal.to_le_bytes())
            }) {
                let data = record.payload;
                let expected = op.calls.get(count).ok_or(E::Selection)?;
                if data.len() != 47
                    || data[46] != 0
                    || u16_at(data, 2)? != count as u16
                    || data[4..20]
                        != c::AssignedIdentity::from_text(expected.contract_id.as_str()).0
                    || data[20..36]
                        != expected
                            .target_kind
                            .as_ref()
                            .map_or([0; 16], |k| c::AssignedIdentity::from_text(k.as_str()).0)
                    || u16_at(data, 36)? != expected.maximum_in_flight
                    || data[38..42] != expected.maximum_input_bytes.to_le_bytes()
                    || data[42..46] != expected.maximum_output_bytes.to_le_bytes()
                {
                    return Err(E::Selection);
                }
                count += 1;
            }
            if count != op.calls.len() {
                return Err(E::Selection);
            }
            indices[usize::from(ordinal)] = Some(index);
        }
        if seen.contains(&false) || indices.iter().flatten().count() != 3 {
            return Err(E::Selection);
        }
        Ok(core::array::from_fn(|i| {
            indices[i].map(|index| {
                self.operations[index]
                    .prepared
                    .take()
                    .expect("exclusive complete preflight")
            })
        }))
    }
}
fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, SoftwareRefusal> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(SoftwareRefusal::Selection)?
            .try_into()
            .map_err(|_| SoftwareRefusal::Selection)?,
    ))
}
pub struct CompactOutputOffers<'o, 'm> {
    owner: &'o CompactOutputSoftware<'m>,
}
impl CompactOutputOffers<'_, '_> {
    pub fn host(&self) -> &c::HostId {
        &self.owner.host
    }
    pub fn boot(&self) -> &c::BootId {
        &self.owner.boot
    }
    pub fn generation(&self) -> c::OfferGeneration {
        self.owner.generation.expect("current offers")
    }
    pub fn source(&self) -> &c::SourceDocumentId {
        &self.owner.source.source_document_id
    }
    pub fn checked(&self) -> &c::CheckedPlotId {
        &self.owner.source.checked_plot_id
    }
    pub fn expanded(&self) -> &c::ExpandedPlotId {
        &self.owner.source.expanded_plot_id
    }
    pub fn iter(&self) -> impl Iterator<Item = &[u8]> {
        self.owner
            .operations
            .iter()
            .filter(|op| op.prepared.is_some())
            .map(|op| op.offer)
    }
}
