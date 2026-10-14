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

#[cfg(test)]
mod tests {
    use super::*;
    fn port() -> LoweredForePort {
        LoweredForePort {
            front_port_id: "result".into(),
            direction: PortDirection::Output,
            track: conduit_core::ConnectionTrack::Payload,
            endpoint: conduit_kernel::RemoteEndpointId(0),
            cord: conduit_kernel::CordId(0),
            value_kind: "value/u64".into(),
            value_contract: None,
            abnormal_kind: None,
            temporal: PortTemporal::Value,
            item_capacity: 1,
            byte_capacity: 8,
            selected_line: None,
        }
    }
    #[test]
    fn successful_owned_adapter_and_report_delivery_allocate_only_before_play() {
        let port = port();
        let mut prepared = PreparedValueForeOutputs::prepare(core::slice::from_ref(&port));
        let probe = crate::allocation_probe::begin();
        let (report, adapter) = prepared.take(&port, 0, &7u64.to_le_bytes()).unwrap();
        assert_eq!(probe.finish(), 0);
        assert_eq!(report, adapter);
        assert_eq!(report.bytes, 7u64.to_le_bytes());
        assert_eq!(report.front_port_id, port.front_port_id);
    }
    #[test]
    fn repeated_value_or_oversized_payload_refuses_without_replay() {
        let port = port();
        let mut prepared = PreparedValueForeOutputs::prepare(core::slice::from_ref(&port));
        prepared.take(&port, 0, &[0; 8]).unwrap();
        assert!(prepared.take(&port, 1, &[0; 8]).is_err());
        let mut prepared = PreparedValueForeOutputs::prepare(core::slice::from_ref(&port));
        assert!(prepared.take(&port, 0, &[0; 9]).is_err());
    }
}
