//! Actual heap payload for the closed local native Mask profile, not wire size.
//! Unsupported owners refuse; empty vectors still retain their spare allocation.
use alloc::{string::String, vec::Vec};
use conduit_core::*;
use conduit_face::PlannedMaskPlot;
use conduit_plan_lowering::lowering::LoweredForePort;
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
    fn vector<T>(&mut self, values: &Vec<T>) -> Option<()> {
        self.add(values.capacity().checked_mul(size_of::<T>())?)
    }
    fn absent<T>(&mut self, values: &Vec<T>) -> Option<()> {
        if !values.is_empty() {
            return None;
        }
        self.vector(values)
    }
}
/// Owned payload only, excluding inline Self, allocator overhead, kernel stores,
/// transient lowering workspace and scenes. This does not validate/admit a Plan.
/// The current four-Gear, single-fragment, local closure is deliberately finite.
pub fn native_mask_plan_backing_bytes(mask: &PlannedMaskPlot) -> Option<usize> {
    let mut n = Bytes::default();
    let m = &mask.mask;
    n.ids(&[
        &m.plot_identity.source_document_id.0,
        &m.plot_identity.checked_plot_id.0,
        &m.plot_identity.expanded_plot_id.0,
        &m.plot_name,
    ])?;
    for port in [&m.face_input, &m.interaction_output, &m.show_output] {
        n.ids(&[&port.front_port_id.0, &port.gear_id.0, &port.gear_port_id.0])?;
    }
    let p = &mask.plan;
    if p.fragments.len() != 1 || p.fragments[0].placements.len() != 4 {
        return None;
    }
    n.ids(&[
        &p.plan_id.0,
        &p.source_document_id.0,
        &p.checked_plot_id.0,
        &p.expanded_plot_id.0,
    ])?;
    n.absent(&p.realization_backs)?;
    n.absent(&p.activations)?;
    n.absent(&p.activation_preparations)?;
    n.vector(&p.fragments)?;
    for f in &p.fragments {
        n.ids(&[
            &f.plan_id.0,
            &f.fragment_id.0,
            &f.source_document_id.0,
            &f.checked_plot_id.0,
            &f.expanded_plot_id.0,
            &f.host_id.0,
            &f.boot_id.0,
        ])?;
        n.absent(&f.realization_backs)?;
        n.absent(&f.execution_regions)?;
        n.absent(&f.execution_fusions)?;
        n.absent(&f.states)?;
        n.absent(&f.shared_pools)?;
        n.vector(&f.placements)?;
        for gear in &f.placements {
            gear_backing(&mut n, gear)?;
        }
        n.vector(&f.connections)?;
        for c in &f.connections {
            n.ids(&[
                &c.connection_id.0,
                &c.source_placement_id.0,
                &c.source_port_id.0,
                &c.sink_placement_id.0,
                &c.sink_port_id.0,
                &c.value_kind.0,
            ])?;
            if let Some(kind) = &c.abnormal_kind {
                n.add(kind.0.capacity())?;
            }
            if c.selected_line.is_some() {
                return None;
            }
            n.absent(&c.admitted_lines)?;
            if let Some(resource) = &c.resource {
                resource_contract(&mut n, &resource.contract)?;
                n.add(resource.owner_placement_id.0.capacity())?;
                resource_binding(&mut n, &resource.source_binding)?;
            }
        }
        n.vector(&f.fore_ports)?;
        for port in &f.fore_ports {
            n.ids(&[
                &port.front_port_id.0,
                &port.placement_id.0,
                &port.gear_port_id.0,
                &port.value_kind.0,
            ])?;
            if let Some(contract) = &port.value_contract {
                n.add(contract.retained_backing_bytes()?)?;
            }
            if let Some(kind) = &port.abnormal_kind {
                n.add(kind.0.capacity())?;
            }
            if let Some(line) = &port.selected_line {
                line_backing(&mut n, line)?;
            }
        }
        n.vector(&f.startup_dependencies)?;
        for d in &f.startup_dependencies {
            n.ids(&[&d.prerequisite_placement_id.0, &d.dependent_placement_id.0])?;
        }
        n.vector(&f.startup_order)?;
        for id in &f.startup_order {
            n.add(id.0.capacity())?;
        }
        n.vector(&f.expected_terminals)?;
        for terminal in &f.expected_terminals {
            match terminal {
                ExpectedTerminal::PlacementCompleted(id) => n.add(id.0.capacity())?,
                ExpectedTerminal::ConnectionCompleted(id) => n.add(id.0.capacity())?,
                ExpectedTerminal::PlanCompleted => (),
            }
        }
        n.vector(&f.expected_sign)?;
        for sign in &f.expected_sign {
            match sign {
                ExpectedSign::PlacementPrepared(id) | ExpectedSign::PlacementTerminal(id) => {
                    n.add(id.0.capacity())?
                }
                ExpectedSign::ConnectionTerminal(id) => n.add(id.0.capacity())?,
                ExpectedSign::PlanFragmentReceived | ExpectedSign::PlanTerminal => (),
            }
        }
        n.vector(&f.plan_fragments)?;
        for commitment in &f.plan_fragments {
            n.ids(&[&commitment.host_id.0, &commitment.fragment_id.0])?;
        }
    }
    Some(n.0)
}
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
fn resource_contract(n: &mut Bytes, port: &ResourcePortContract) -> Option<()> {
    n.ids(&[&port.port_id.0, &port.class_id.0])
}
fn resource_binding(n: &mut Bytes, binding: &ResourceBinding) -> Option<()> {
    if binding.protected.is_some() || binding.compute.is_some() || binding.content.is_some() {
        return None;
    }
    n.ids(&[&binding.pool_id.0, &binding.class_id.0])
}
fn config_value(n: &mut Bytes, value: &ConfigurationValue) -> Option<()> {
    match value {
        ConfigurationValue::Bool(_) | ConfigurationValue::U64(_) | ConfigurationValue::I64(_) => {
            Some(())
        }
        ConfigurationValue::Text(text) => n.add(text.capacity()),
        _ => None,
    }
}
fn semantic_contract(n: &mut Bytes, contract: &KindSemanticContract) -> Option<()> {
    n.vector(&contract.configuration)?;
    for field in &contract.configuration {
        n.add(field.key.capacity())?;
        config_value(n, &field.default_value)?;
        match &field.rule {
            KindConfigurationRule::Any
            | KindConfigurationRule::U64Range { .. }
            | KindConfigurationRule::I64Range { .. }
            | KindConfigurationRule::DurationMillis { .. }
            | KindConfigurationRule::TextBytes { .. }
            | KindConfigurationRule::DataLabel { .. } => (),
            _ => return None,
        }
    }
    n.vector(&contract.laws)?;
    for law in &contract.laws {
        match law {
            KindSemanticLaw::Terminal(_)
            | KindSemanticLaw::ExternalEffects(_)
            | KindSemanticLaw::TemporalState(_)
            | KindSemanticLaw::TimeDependence(_)
            | KindSemanticLaw::RandomDependence(_)
            | KindSemanticLaw::ResourceDependence(_)
            | KindSemanticLaw::Suspension(_)
            | KindSemanticLaw::Variability(_) => (),
            KindSemanticLaw::Replay(ReplayBehavior::Exact | ReplayBehavior::Ineligible) => (),
            KindSemanticLaw::ResourcePorts(ports) => {
                n.vector(ports)?;
                for port in ports {
                    resource_contract(n, port)?;
                }
            }
            KindSemanticLaw::ValueContracts(values) => {
                n.vector(values)?;
                for value in values {
                    match &value.location {
                        FrontValueLocation::Startup(name) => n.add(name.capacity())?,
                        FrontValueLocation::Input(id)
                        | FrontValueLocation::Output(id)
                        | FrontValueLocation::InputAbnormal(id)
                        | FrontValueLocation::OutputAbnormal(id) => n.add(id.0.capacity())?,
                    }
                    n.add(value.contract.retained_backing_bytes()?)?;
                }
            }
            _ => return None,
        }
    }
    Some(())
}
fn gear_backing(n: &mut Bytes, g: &PlannedGear) -> Option<()> {
    n.ids(&[
        &g.placement_id.0,
        &g.gear_id.0,
        &g.kind_id.0,
        &g.kind_contract_revision.0,
        &g.execution_profile_id.0,
        &g.host_id.0,
        &g.boot_id.0,
        &g.capability_id.0,
        &g.implementation_id.0,
        &g.artifact_id.0,
    ])?;
    if let Some(base) = &g.base {
        n.ids(&[
            &base.base_id.0,
            &base.provider_instance_id.0,
            &base.implementation_id.0,
            &base.mechanism_family.0,
        ])?;
    }
    n.vector(&g.configuration)?;
    for c in &g.configuration {
        n.add(c.key.capacity())?;
        config_value(n, &c.value)?;
    }
    n.absent(&g.realization_characteristics)?;
    n.absent(&g.realization_properties)?;
    n.absent(&g.terminal_transductions)?;
    n.absent(&g.pool_references)?;
    for ports in [&g.inputs, &g.outputs] {
        n.vector(ports)?;
        for port in ports {
            n.ids(&[&port.port_id.0, &port.value_kind.0])?;
            if let Some(kind) = &port.abnormal_kind {
                n.add(kind.0.capacity())?;
            }
        }
    }
    semantic_contract(n, &g.semantic_contract)?;
    n.vector(&g.host_calls)?;
    for call in &g.host_calls {
        n.add(call.contract_id.0.capacity())?;
        if let Some(kind) = &call.target_kind {
            n.add(kind.0.capacity())?;
        }
    }
    n.vector(&g.resources)?;
    for resource in &g.resources {
        resource_binding(n, resource)?;
    }
    n.vector(&g.authority)?;
    for a in &g.authority {
        n.ids(&[
            &a.grant_id.0,
            &a.contract_id.0,
            &a.host_call_contract_id.0,
            &a.subject_kind.0,
            &a.host_id.0,
            &a.boot_id.0,
            &a.capability_id.0,
        ])?;
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
