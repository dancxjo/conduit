//! Actual heap payload for the closed local native Mask profile, not wire size.
//! Unsupported owners refuse; empty vectors still retain their spare allocation.
use alloc::string::String;
#[cfg(test)]
use alloc::vec::Vec;
use conduit_core::*;
#[cfg(test)]
use conduit_face::PlannedMaskPlot;
use conduit_plan_lowering::lowering::LoweredForePort;
#[cfg(test)]
use core::mem::size_of;

#[derive(Default)]
struct Bytes(usize);
impl Bytes {
    fn add(&mut self, bytes: usize) -> Option<()> {
        self.0 = self.0.checked_add(bytes)?;
        Some(())
    }
    fn ids(&mut self, strings: &[&String]) -> Option<()> {
        for value in strings {
            self.add(value.capacity())?;
        }
        Some(())
    }
}
pub use conduit_face::native_mask_plan_backing_bytes;
pub(super) fn lowered_fore_backing_bytes(port: &LoweredForePort) -> Option<usize> {
    let mut n = Bytes::default();
    if let Some(line) = &port.selected_line {
        line_backing(&mut n, line)?;
    }
    n.ids(&[&port.front_port_id.0, &port.value_kind.0])?;
    if let Some(kind) = &port.abnormal_kind {
        n.add(kind.0.capacity())?;
    }
    if let Some(contract) = &port.value_contract {
        n.add(contract.retained_backing_bytes()?)?;
    }
    Some(n.0)
}
// Admitted route metadata is owned here; availability and transport buffers
// are separate owners. Counting it grants neither a Line nor effect authority.
fn line_backing(n: &mut Bytes, line: &AdmittedLine) -> Option<()> {
    let b = &line.binding;
    n.ids(&[
        &line.line_id.0,
        &b.binding_id.0,
        &b.base.0,
        &b.base_instance_id.0,
    ])?;
    for endpoint in [&b.source, &b.sink] {
        n.ids(&[
            &endpoint.host_id.0,
            &endpoint.boot_id.0,
            &endpoint.endpoint_id.0,
        ])?;
    }
    if let LinkCredentialReference::Opaque(id) = &b.credential {
        n.add(id.0.capacity())?;
    }
    if let LinkAuthorityReference::Grant(id) = &b.authority {
        n.add(id.0.capacity())?;
    }
    Some(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn mask() -> PlannedMaskPlot {
        crate::mask_control::prepare_stage(
            crate::mask_control::Adapter::Native,
            &"host/owned-plan".into(),
            &"boot/owned-plan".into(),
            1,
            None,
        )
        .unwrap()
        .planned_mask
    }
    #[test]
    fn ordinary_mask_owner_counts_spare_capacity_without_serializing() {
        let mut mask = mask();
        let initial = native_mask_plan_backing_bytes(&mask).expect("ordinary four-Gear closure");
        let old = mask.mask.plot_name.capacity();
        mask.mask.plot_name.reserve(127);
        let delta = mask.mask.plot_name.capacity() - old;
        assert_eq!(native_mask_plan_backing_bytes(&mask), Some(initial + delta));
        mask.plan.activations = Vec::with_capacity(7);
        assert_eq!(
            native_mask_plan_backing_bytes(&mask),
            Some(
                initial
                    + delta
                    + mask.plan.activations.capacity() * size_of::<PlannedActivationEntry>()
            )
        );
    }
    #[test]
    fn unsupported_dynamic_owners_refuse_instead_of_undercharging() {
        let mut mask = mask();
        mask.plan.fragments[0].placements[0]
            .configuration
            .push(ConfigurationEntry {
                key: "unsupported-unit".into(),
                value: ConfigurationValue::Text(String::new()),
            });
        assert!(native_mask_plan_backing_bytes(&mask).is_some());
        mask.plan.fragments.push(mask.plan.fragments[0].clone());
        assert_eq!(native_mask_plan_backing_bytes(&mask), None);
    }
    #[test]
    fn retained_fore_charges_contract_and_spare_identity_payload() {
        let mask = mask();
        let lowered =
            conduit_plan_lowering::lowering::lower_plan_fragment(&mask.plan.fragments[0]).unwrap();
        let mut fore = lowered.fore_ports[0].clone();
        let initial = lowered_fore_backing_bytes(&fore).unwrap();
        let old = fore.front_port_id.0.capacity();
        fore.front_port_id.0.reserve(91);
        assert_eq!(
            lowered_fore_backing_bytes(&fore),
            Some(initial + fore.front_port_id.0.capacity() - old)
        );
    }
    #[test]
    fn routed_fore_retains_bound_link_ids_credentials_and_spare_capacity() {
        // Descriptive ownership fixture, not transport admission evidence.
        let mut mask = mask();
        let mut lowered =
            conduit_plan_lowering::lowering::lower_plan_fragment(&mask.plan.fragments[0]).unwrap();
        let mut line = AdmittedLine {
            line_id: "line/owned".into(),
            binding: BoundLink {
                binding_id: "binding/owned".into(),
                source: LinkEndpoint {
                    host_id: "host/source".into(),
                    boot_id: "boot/source".into(),
                    endpoint_id: "endpoint/source".into(),
                },
                sink: LinkEndpoint {
                    host_id: "host/sink".into(),
                    boot_id: "boot/sink".into(),
                    endpoint_id: "endpoint/sink".into(),
                },
                base: "base/owned".into(),
                base_instance_id: "instance/owned".into(),
                credential: LinkCredentialReference::Opaque("credential/owned".into()),
                authority: LinkAuthorityReference::Grant("grant/owned".into()),
                limits: LinkLimits {
                    maximum_in_flight_items: 1,
                    maximum_payload_bytes: 128,
                    maximum_buffered_bytes: 128,
                    maximum_frame_bytes: 256,
                },
            },
            contract: LineContract {
                scope: LineScope::RoutedNetwork,
                traffic_shape: LineTrafficShape::ByteStream,
                duplex: LineDuplex::FullDuplex,
                ordering: LineOrdering::Ordered,
                reliability: LineReliability::Reliable,
                continuation: LineContinuation::None,
                security: LineSecurity::AuthenticatedEncrypted,
            },
        };
        line.binding.source.endpoint_id.0.reserve(151);
        let b = &line.binding;
        let expected: usize = [
            &line.line_id.0,
            &b.binding_id.0,
            &b.base.0,
            &b.base_instance_id.0,
            &b.source.host_id.0,
            &b.source.boot_id.0,
            &b.source.endpoint_id.0,
            &b.sink.host_id.0,
            &b.sink.boot_id.0,
            &b.sink.endpoint_id.0,
        ]
        .iter()
        .map(|s| s.capacity())
        .sum::<usize>()
            + match &b.credential {
                LinkCredentialReference::Opaque(id) => id.0.capacity(),
                _ => 0,
            }
            + match &b.authority {
                LinkAuthorityReference::Grant(id) => id.0.capacity(),
                _ => 0,
            };
        let initial = native_mask_plan_backing_bytes(&mask).unwrap();
        let fore_initial = lowered_fore_backing_bytes(&lowered.fore_ports[0]).unwrap();
        // Move rather than clone into each tested owner: Clone need not preserve
        // a String's spare capacity, so each allocation must be measured itself.
        mask.plan.fragments[0].fore_ports[0].selected_line = Some(line);
        assert_eq!(
            native_mask_plan_backing_bytes(&mask),
            Some(initial + expected)
        );
        lowered.fore_ports[0].selected_line =
            mask.plan.fragments[0].fore_ports[0].selected_line.take();
        assert_eq!(
            lowered_fore_backing_bytes(&lowered.fore_ports[0]),
            Some(fore_initial + expected)
        );
    }
}
