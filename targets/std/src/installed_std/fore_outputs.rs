//! Prepared per-port identities and one-value storage for external Fore output.
use crate::{ExternalForeDelivery, ExternalForeIdentity};
use conduit_core::{PortDirection, PortTemporal};
use conduit_plan_lowering::lowering::LoweredForePort;
use std::sync::Arc;

struct OutputSlot<'a> {
    port: &'a LoweredForePort,
    identity: Arc<ExternalForeIdentity>,
    value: Option<ExternalForeDelivery>,
}
pub(super) struct PreparedForeOutputs<'a> {
    slots: Vec<OutputSlot<'a>>,
}
impl<'a> PreparedForeOutputs<'a> {
    pub(super) fn prepare(ports: &'a [LoweredForePort]) -> Self {
        let slots = ports
            .iter()
            .filter(|p| p.direction == PortDirection::Output)
            .map(|p| {
                let identity = Arc::new(ExternalForeIdentity {
                    front_port_id: p.front_port_id.clone(),
                    track: p.track,
                    value_kind: p.value_kind.clone(),
                });
                let value = (p.temporal == PortTemporal::Value).then(|| {
                    ExternalForeDelivery::prepared(identity.clone(), p.byte_capacity as usize)
                });
                OutputSlot {
                    port: p,
                    identity,
                    value,
                }
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
            .find(|slot| std::ptr::eq(slot.port, port))
            .ok_or("Value Fore output has no prepared port")?;
        let mut report = slot
            .value
            .take()
            .ok_or("Value Fore output exceeded its admitted one-value scope")?;
        report.fill_prepared(sequence, bytes)?;
        let adapter = report.clone();
        Ok((report, adapter))
    }
    pub(super) fn stream(
        &self,
        port: &LoweredForePort,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<ExternalForeDelivery, String> {
        if !matches!(
            port.temporal,
            PortTemporal::Flow { .. } | PortTemporal::Current
        ) || bytes.len() > port.byte_capacity as usize
        {
            return Err("Fore output exceeded its admitted temporal or byte bound".into());
        }
        let slot = self
            .slots
            .iter()
            .find(|slot| std::ptr::eq(slot.port, port))
            .ok_or("Fore output has no prepared stream port")?;
        Ok(ExternalForeDelivery::with_identity(
            slot.identity.clone(),
            sequence,
            bytes.into(),
        ))
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
            quantity_profile: conduit_core::QuantityValueProfile::Decimal,
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
        let mut prepared = PreparedForeOutputs::prepare(core::slice::from_ref(&port));
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
        let mut prepared = PreparedForeOutputs::prepare(core::slice::from_ref(&port));
        prepared.take(&port, 0, &[0; 8]).unwrap();
        assert!(prepared.take(&port, 1, &[0; 8]).is_err());
        let mut prepared = PreparedForeOutputs::prepare(core::slice::from_ref(&port));
        assert!(prepared.take(&port, 0, &[0; 9]).is_err());
    }

    #[test]
    fn only_the_exact_borrowed_prepared_port_can_consume_its_storage() {
        let port = port();
        let mut prepared = PreparedForeOutputs::prepare(core::slice::from_ref(&port));
        let mut foreign = port.clone();
        foreign.value_kind = "value/text".into();
        assert!(prepared.take(&foreign, 0, &[0; 8]).is_err());
        let (report, adapter) = prepared.take(&port, 0, &[0; 8]).unwrap();
        assert_eq!(report, adapter);
        assert_eq!(report.value_kind, port.value_kind);
    }

    #[test]
    fn flow_and_current_outputs_keep_independent_values_with_one_shared_port_identity() {
        for temporal in [PortTemporal::Flow { closes: true }, PortTemporal::Current] {
            let mut port = port();
            port.temporal = temporal;
            let prepared = PreparedForeOutputs::prepare(core::slice::from_ref(&port));
            let first = prepared.stream(&port, 3, &[1; 8]).unwrap();
            let second = prepared.stream(&port, 4, &[2; 8]).unwrap();
            assert!(std::ptr::eq(&*first, &*second));
            assert_ne!(first.bytes.as_ptr(), second.bytes.as_ptr());
            assert_eq!(first.sequence, 3);
            assert_eq!(second.sequence, 4);
            assert_eq!(first.bytes, [1; 8]);
            assert_eq!(second.bytes, [2; 8]);
            let retained = first.clone();
            assert_eq!(first.bytes.as_ptr(), retained.bytes.as_ptr());
            assert!(prepared.stream(&port, 5, &[0; 9]).is_err());
            assert_eq!(first.bytes, [1; 8]);
            assert_eq!(second.bytes, [2; 8]);
        }
    }
}
