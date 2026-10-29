//! Exclusive transfer of initialized portable output owners from a validated
//! compact assignment. No rich Plan, new provider or native authority is made.
use super::*;
use conduit_kernel::NodeId;
pub(crate) struct AssignedOutputSoftware<'a> {
    owner: &'a mut ReadySoftwareBacks,
    indices: [Option<usize>; 4],
}
impl ReadySoftwareBacks {
    pub(crate) fn assigned_output<'a>(
        &'a mut self,
        identity: c::AssignedPlanView,
        generation: c::OfferGeneration,
        source: &c::PlotIdentity,
        records: &[c::AssignedRecord<'_>],
        write: NodeId,
    ) -> Result<AssignedOutputSoftware<'a>, SoftwareRefusal> {
        use SoftwareRefusal as E;
        if identity.host != c::AssignedIdentity::from_text(self.host.as_str())
            || identity.boot != c::AssignedIdentity::from_text(self.boot.as_str())
            || generation.0 == 0
            || self.generation != Some(generation)
            || source.source_document_id != self.source
            || source.checked_plot_id != self.checked
            || source.expanded_plot_id != self.expanded
            || write.0 >= 4
        {
            return Err(E::Selection);
        }
        if self.metadata_released {
            return Err(E::AlreadySelected);
        }
        if self.operations.len() != 3 {
            return Err(E::Selection);
        }
        let mut indices = [None; 4];
        let mut seen = [false; 4];
        for node in records.iter().filter(|r| r.tag == c::ASSIGNED_NODE) {
            let data = node.payload;
            if data.len() != 52 {
                return Err(E::Selection);
            }
            let ordinal = usize::from(u16_at(data, 0)?);
            let seen = seen.get_mut(ordinal).ok_or(E::Selection)?;
            if *seen {
                return Err(E::Selection);
            }
            *seen = true;
            if ordinal == usize::from(write.0) {
                if data[4..20] != c::AssignedIdentity::from_text(conduit_tm1637_wire::WRITE_KIND).0
                    || records.iter().any(|r| {
                        r.tag == c::ASSIGNED_CONFIGURATION
                            && r.payload.get(..2) == Some(&write.0.to_le_bytes())
                    })
                {
                    return Err(E::Selection);
                }
                continue;
            }
            let index = self
                .operations
                .iter()
                .position(|op| {
                    data[4..20] == c::AssignedIdentity::from_text(op.offer.kind_id.as_str()).0
                        && data[20..36]
                            == c::AssignedIdentity::from_text(
                                op.offer.implementation.implementation_id.as_str(),
                            )
                            .0
                        && data[36..52]
                            == c::AssignedIdentity::from_text(
                                op.offer.implementation.artifact_id.as_str(),
                            )
                            .0
                })
                .ok_or(E::Selection)?;
            if indices.contains(&Some(index)) {
                return Err(E::Selection);
            }
            let op = &self.operations[index];
            if op.prepared.is_none() {
                return Err(E::AlreadySelected);
            }
            let [configuration] = op.configuration.as_slice() else {
                return Err(E::Unsupported);
            };
            let c::ConfigurationValue::Text(text) = &configuration.value else {
                return Err(E::Unsupported);
            };
            if !matches!(configuration.key.as_str(), "program" | "selector") {
                return Err(E::Unsupported);
            }
            let mut configs = records.iter().filter(|r| {
                r.tag == c::ASSIGNED_CONFIGURATION
                    && r.payload.get(..2) == Some(&(ordinal as u16).to_le_bytes())
            });
            let data = configs.next().ok_or(E::Selection)?.payload;
            if configs.next().is_some()
                || data.len() < 19
                || data[2..18] != c::AssignedIdentity::from_text(&configuration.key).0
                || data[18] != 3
                || &data[19..] != text.as_bytes()
            {
                return Err(E::Selection);
            }
            let mut count = 0;
            for record in records.iter().filter(|r| {
                r.tag == c::ASSIGNED_HOST_CALL
                    && r.payload.get(..2) == Some(&(ordinal as u16).to_le_bytes())
            }) {
                let data = record.payload;
                let expected = op.offer.host_calls.get(count).ok_or(E::Selection)?;
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
                    || u32_at(data, 38)? != expected.maximum_input_bytes
                    || u32_at(data, 42)? != expected.maximum_output_bytes
                {
                    return Err(E::Selection);
                }
                count += 1;
            }
            if count != op.offer.host_calls.len() {
                return Err(E::Selection);
            }
            indices[ordinal] = Some(index);
        }
        if seen.contains(&false) || indices.iter().flatten().count() != 3 {
            return Err(E::Selection);
        }
        Ok(AssignedOutputSoftware {
            owner: self,
            indices,
        })
    }
}
impl AssignedOutputSoftware<'_> {
    pub(crate) fn take(self) -> [Option<PreparedSoftwareOperation>; 4] {
        let operations = core::array::from_fn(|i| {
            self.indices[i].map(|index| {
                self.owner.operations[index]
                    .prepared
                    .take()
                    .expect("exclusively preflighted software")
            })
        });
        // assigned_output preflight requires all three owned operations exactly
        // once. Retire their descriptors before allocating the scheduler box.
        self.owner
            .release_transferred_metadata()
            .expect("complete preflighted output transfer");
        operations
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
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, SoftwareRefusal> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(SoftwareRefusal::Selection)?
            .try_into()
            .map_err(|_| SoftwareRefusal::Selection)?,
    ))
}
