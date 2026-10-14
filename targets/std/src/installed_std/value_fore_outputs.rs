//! Owned Value Fore deliveries prepared before Play; no metadata/byte clones in Play.
use crate::ExternalForeDelivery;
use conduit_core::{PortDirection, PortTemporal};
use conduit_plan_lowering::lowering::LoweredForePort;
pub(super) struct PreparedValueForeOutputs {
    slots: Vec<
        Option<(
            conduit_kernel::RemoteEndpointId,
            conduit_kernel::CordId,
            ExternalForeDelivery,
            ExternalForeDelivery,
        )>,
    >,
}
impl PreparedValueForeOutputs {
    pub(super) fn prepare(ports: &[LoweredForePort]) -> Self {
        let slots = ports
            .iter()
            .filter(|p| p.direction == PortDirection::Output && p.temporal == PortTemporal::Value)
            .map(|p| {
                let delivery = || ExternalForeDelivery {
                    front_port_id: p.front_port_id.clone(),
                    track: p.track,
                    value_kind: p.value_kind.clone(),
                    sequence: 0,
                    bytes: Vec::with_capacity(p.byte_capacity as usize),
                };
                Some((p.endpoint, p.cord, delivery(), delivery()))
            })
            .collect();
        Self { slots }
    }
    pub(super) fn take(
        &mut self,
        port: &LoweredForePort,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<(ExternalForeDelivery, ExternalForeDelivery), String> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| {
                slot.as_ref().is_some_and(|(endpoint, cord, _, _)| {
                    *endpoint == port.endpoint && *cord == port.cord
                })
            })
            .ok_or("Value Fore output exceeded its admitted one-value scope")?;
        let (_, _, mut report, mut adapter) = slot.take().expect("private prepared slot");
        if bytes.len() > report.bytes.capacity() || bytes.len() > adapter.bytes.capacity() {
            return Err("Value Fore output exceeded its pre-Play byte bound".into());
        }
        report.sequence = sequence;
        adapter.sequence = sequence;
        report.bytes.extend_from_slice(bytes);
        adapter.bytes.extend_from_slice(bytes);
        Ok((report, adapter))
    }
}
