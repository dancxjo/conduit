//! Prepared per-port identities and explicit finite output custody.
use crate::{ExternalForeDelivery, ExternalForeIdentity};
use conduit_core::{PortDirection, PortTemporal};
use conduit_plan_lowering::lowering::LoweredForePort;
use std::sync::Arc;

struct OutputSlot<'a> {
    port: &'a LoweredForePort,
    identity: Arc<ExternalForeIdentity>,
    value: Option<ExternalForeDelivery>,
    current: Vec<Option<ExternalForeDelivery>>,
    next_current: usize,
}
pub(super) struct PreparedForeOutputs<'a> {
    slots: Vec<OutputSlot<'a>>,
    report_capacity: usize,
}
impl<'a> PreparedForeOutputs<'a> {
    pub(super) fn prepare(ports: &'a [LoweredForePort]) -> Self {
        Self::prepare_outputs(ports, None)
    }
    /// Finite caller-selected Current lifetime, independent of the planned queue. Slots are never
    /// recycled: report or adapter custody can outlive the next delivery.
    pub(super) fn prepare_bounded_current(
        ports: &'a [LoweredForePort],
        current_output_capacity: u16,
    ) -> Self {
        Self::prepare_outputs(ports, Some(current_output_capacity))
    }
    pub(super) fn report_capacity(&self) -> usize {
        self.report_capacity
    }
    fn prepare_outputs(ports: &'a [LoweredForePort], current_output_capacity: Option<u16>) -> Self {
        let mut report_capacity = 0;
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
                let current_items = if p.temporal == PortTemporal::Current {
                    current_output_capacity.map(usize::from).unwrap_or(0)
                } else {
                    0
                };
                let current = (0..current_items)
                    .map(|_| {
                        Some(ExternalForeDelivery::prepared(
                            identity.clone(),
                            p.byte_capacity as usize,
                        ))
                    })
                    .collect();
                report_capacity += match p.temporal {
                    PortTemporal::Value => 1,
                    PortTemporal::Current => {
                        current_output_capacity.unwrap_or(p.item_capacity) as usize
                    }
                    _ => p.item_capacity as usize,
                };
                OutputSlot {
                    port: p,
                    identity,
                    value,
                    current,
                    next_current: 0,
                }
            })
            .collect();
        Self {
            slots,
            report_capacity,
        }
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
    pub(super) fn take_current(
        &mut self,
        port: &LoweredForePort,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<(ExternalForeDelivery, ExternalForeDelivery), &'static str> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| std::ptr::eq(slot.port, port))
            .ok_or("Current Fore output has no prepared port")?;
        if port.temporal != PortTemporal::Current || bytes.len() > port.byte_capacity as usize {
            return Err("Current Fore output exceeded its admitted temporal or byte bound");
        }
        let mut report = slot
            .current
            .get_mut(slot.next_current)
            .and_then(Option::take)
            .ok_or("Current Fore output exceeded its admitted lifetime scope")?;
        // Each slot is uniquely owned and its byte bound was checked above.
        report
            .fill_prepared(sequence, bytes)
            .expect("unescaped prepared Current slot");
        slot.next_current += 1;
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
    #[test]
    fn bounded_current_preserves_escaped_custody_and_refuses_without_allocation() {
        let mut port = port();
        port.temporal = PortTemporal::Current;
        port.item_capacity = 1;
        let mut prepared =
            PreparedForeOutputs::prepare_bounded_current(core::slice::from_ref(&port), 2);
        assert_eq!(prepared.report_capacity(), 2);
        let foreign = port.clone();
        let probe = crate::allocation_probe::begin();
        assert!(prepared.take_current(&foreign, 0, &[0; 8]).is_err());
        assert!(prepared.take_current(&port, 0, &[0; 9]).is_err());
        let (first, adapter) = prepared.take_current(&port, 3, &[1; 8]).unwrap();
        let retained = adapter.clone();
        drop(adapter);
        let (second, second_adapter) = prepared.take_current(&port, 4, &[2; 8]).unwrap();
        assert_eq!(first.bytes, [1; 8]);
        assert_eq!(retained.bytes, [1; 8]);
        assert_eq!(second.bytes, [2; 8]);
        assert_eq!(second, second_adapter);
        assert_eq!(first.sequence, 3);
        assert_eq!(second.sequence, 4);
        assert_ne!(first.bytes.as_ptr(), second.bytes.as_ptr());
        assert!(prepared.take_current(&port, 5, &[3; 8]).is_err());
        drop(second);
        drop(second_adapter);
        assert!(prepared.take_current(&port, 6, &[4; 8]).is_err());
        assert_eq!(probe.finish(), 0);
    }

    #[test]
    fn current_lifetime_is_independent_of_queue_and_keeps_value_one_shot() {
        let mut current = port();
        current.temporal = PortTemporal::Current;
        current.item_capacity = 3;
        let ports = [current, port()];
        let mut prepared = PreparedForeOutputs::prepare_bounded_current(&ports, 1);
        assert_eq!(prepared.report_capacity(), 2);
        let probe = crate::allocation_probe::begin();
        for sequence in 0..1 {
            prepared.take_current(&ports[0], sequence, &[0; 8]).unwrap();
        }
        assert!(prepared.take_current(&ports[0], 3, &[0; 8]).is_err());
        prepared.take(&ports[1], 0, &[1; 8]).unwrap();
        assert!(prepared.take_current(&ports[1], 1, &[0; 8]).is_err());
        assert_eq!(probe.finish(), 0);
    }
    #[test]
    fn lifecycle_signs_cover_selected_current_history_without_changing_other_ports() {
        use super::super::external_fore_feeder::{
            remote_sign_items, remote_sign_items_with_current_capacity,
        };
        let mut current = port();
        current.temporal = PortTemporal::Current;
        let ports = core::slice::from_ref(&current);
        assert_eq!(remote_sign_items(ports, 1).unwrap(), 4);
        assert_eq!(
            remote_sign_items_with_current_capacity(ports, 1, Some(2)).unwrap(),
            7
        );
        // Large input counts do not silently enlarge explicitly selected history.
        assert_eq!(
            remote_sign_items_with_current_capacity(ports, 9, Some(2)).unwrap(),
            7
        );
        current.item_capacity = 3;
        assert_eq!(
            remote_sign_items_with_current_capacity(core::slice::from_ref(&current), 1, Some(1))
                .unwrap(),
            10
        );
        current.temporal = PortTemporal::Flow { closes: true };
        let ports = core::slice::from_ref(&current);
        assert_eq!(
            remote_sign_items_with_current_capacity(ports, 4, Some(9)).unwrap(),
            remote_sign_items(ports, 4).unwrap()
        );
        current.direction = PortDirection::Input;
        let ports = core::slice::from_ref(&current);
        assert_eq!(
            remote_sign_items_with_current_capacity(ports, 4, Some(9)).unwrap(),
            5
        );
    }
}
