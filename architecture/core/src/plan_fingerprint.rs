//! Canonical fingerprints for immutable Plan fragments and their commitment set.
//!
//! This module encodes admitted truth. It neither selects realizations nor
//! executes or mutates Plans.

mod execution_regions;
pub(crate) mod root;
pub use root::{
    compute_plan_fingerprint_ref, FragmentCommitmentRef, FragmentCommitmentRefs,
    PlanFingerprintInputRef,
};
mod semantic_contract;
use semantic_contract::push_semantic_contract;

use crate::canonical_bytes::{CanonicalHash, CanonicalSink};
use crate::{
    characteristic, execution_fusion, plan_realization, push_string, push_u32, push_u64,
    AdmittedLine, BoundLink, CancellationPolicy, CheckedFront, ConfigurationValue,
    FragmentCommitment, FragmentId, LinkAuthorityReference, LinkCredentialReference, PlanFragment,
    PlanId, PlannedActivationEntry, PlannedActivationPreparationBinding, PlotBack, PlotIdentity,
    PortDescriptor, PortDirection, PortTemporal, TerminalPolicy,
};
use alloc::string::String;
#[cfg(test)]
use alloc::vec::Vec;

fn push_optional_string(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    value: Option<&str>,
) {
    match value {
        Some(value) => {
            canonical.push(1);
            push_string(canonical, value);
        }
        None => canonical.push(0),
    }
}

mod fragment;
pub use fragment::{
    compute_fragment_id, FragmentFingerprintBuilder, FragmentFingerprintHeader,
    FragmentFingerprintHeaderRef, FragmentFingerprintRefusal, FragmentFingerprintTail,
    FragmentFingerprintTailRef,
};

pub(crate) fn push_configuration_value(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    value: &ConfigurationValue,
) {
    match value {
        ConfigurationValue::Bool(value) => {
            canonical.push(0);
            canonical.push(u8::from(*value));
        }
        ConfigurationValue::U64(value) => {
            canonical.push(1);
            push_u64(canonical, *value);
        }
        ConfigurationValue::Text(value) => {
            canonical.push(2);
            push_string(canonical, value);
        }
        ConfigurationValue::I64(value) => {
            canonical.push(3);
            canonical.extend_from_slice(&value.to_le_bytes());
        }
        ConfigurationValue::Structured(value) => {
            canonical.push(4);
            push_string(canonical, value.profile().as_str());
            push_u32(canonical, value.canonical_value().len() as u32);
            canonical.extend_from_slice(value.canonical_value());
        }
        ConfigurationValue::Quantity(value) => {
            canonical.push(6);
            canonical.extend_from_slice(value.canonical_value());
            push_string(canonical, value.source());
        }
        ConfigurationValue::TemperatureDifference(value) => {
            canonical.push(8);
            canonical.extend_from_slice(value.canonical_value());
            push_string(canonical, value.source());
        }
        ConfigurationValue::Unit(value) => {
            canonical.push(7);
            canonical.extend_from_slice(value.canonical_value());
            push_string(canonical, value.source());
        }
    }
}

fn push_terminal_transduction(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    profile: &crate::TerminalTransductionProfile,
) {
    use crate::{
        AbnormalTerminalTransduction as Abnormal, CancellationTransduction as Cancellation,
        NormalCloseTransduction as Normal,
    };
    push_string(canonical, profile.input_port_id.as_str());
    push_string(canonical, profile.output_port_id.as_str());
    match &profile.normal_close {
        Normal::NotAccepted => canonical.push(0),
        Normal::PropagateAfterDrain => canonical.push(1),
        Normal::Consume => canonical.push(2),
        Normal::FlushThenPropagate(bound) => {
            canonical.push(3);
            canonical.extend_from_slice(&bound.maximum_items.to_le_bytes());
            push_u32(canonical, bound.maximum_bytes);
        }
        Normal::DomainSpecific { law } => {
            canonical.push(4);
            push_string(canonical, law.as_str());
        }
        Normal::FlushThenPropagateWhenAllClose(bound) => {
            canonical.push(5);
            canonical.extend_from_slice(&bound.maximum_items.to_le_bytes());
            push_u32(canonical, bound.maximum_bytes);
        }
        Normal::PropagateWhenAllClose => canonical.push(6),
    }
    match &profile.abnormal {
        Abnormal::NotAccepted => canonical.push(0),
        Abnormal::PropagateAfterDrain => canonical.push(1),
        Abnormal::Recover => canonical.push(2),
        Abnormal::FinalizeThenPropagate(bound) => {
            canonical.push(3);
            canonical.extend_from_slice(&bound.maximum_items.to_le_bytes());
            push_u32(canonical, bound.maximum_bytes);
        }
        Abnormal::DomainSpecific { law } => {
            canonical.push(4);
            push_string(canonical, law.as_str());
        }
    }
    match &profile.cancellation {
        Cancellation::NotCancellable => canonical.push(0),
        Cancellation::Request { disposition_kind } => {
            canonical.push(1);
            push_string(canonical, disposition_kind.as_str());
        }
        Cancellation::DomainSpecific { law } => {
            canonical.push(2);
            push_string(canonical, law.as_str());
        }
    }
}

fn push_checked_front(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    front: &CheckedFront,
) {
    push_u32(canonical, front.startup_parameters().len() as u32);
    for parameter in front.startup_parameters() {
        push_string(canonical, &parameter.name);
        push_string(canonical, parameter.value_type.as_str());
        canonical.push(u8::from(parameter.has_default));
    }
    push_ports(canonical, front.inputs());
    push_ports(canonical, front.outputs());
    push_u32(canonical, front.resource_ports().len() as u32);
    for resource in front.resource_ports() {
        push_string(canonical, resource.port_id.as_str());
        push_string(canonical, resource.class_id.as_str());
        canonical.push(resource.ownership as u8);
        canonical.push(resource.lifecycle as u8);
        canonical.push(resource.mobility as u8);
    }
    push_u32(canonical, front.value_contracts().len() as u32);
    for value_contract in front.value_contracts() {
        match &value_contract.location {
            crate::FrontValueLocation::Startup(name) => {
                canonical.push(0);
                push_string(canonical, name);
            }
            crate::FrontValueLocation::Input(port) => {
                canonical.push(1);
                push_string(canonical, port.as_str());
            }
            crate::FrontValueLocation::Output(port) => {
                canonical.push(2);
                push_string(canonical, port.as_str());
            }
            crate::FrontValueLocation::InputAbnormal(port) => {
                canonical.push(3);
                push_string(canonical, port.as_str());
            }
            crate::FrontValueLocation::OutputAbnormal(port) => {
                canonical.push(4);
                push_string(canonical, port.as_str());
            }
        }
        push_value_contract(canonical, &value_contract.contract);
    }
    match front.shorthand() {
        Some((input, output)) => {
            canonical.push(1);
            push_string(canonical, input.as_str());
            push_string(canonical, output.as_str());
        }
        None => canonical.push(0),
    }
}

fn push_value_contract(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    contract: &crate::CheckedValueContract,
) {
    let identity = contract.identity_bytes();
    push_u32(canonical, identity.len() as u32);
    canonical.extend_from_slice(&identity);
}

/// Returns the canonical semantic fingerprint of an executable Front.
///
/// Nominal callable names and authoring aliases are deliberately absent. The
/// digest changes only when the exact callable contract changes.
pub fn compute_checked_front_fingerprint(front: &CheckedFront) -> String {
    let mut canonical = CanonicalHash::new();
    push_checked_front(&mut canonical, front);
    crate::hex_digest(&canonical.finish())
}

fn push_bound_link(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    binding: &BoundLink,
) {
    push_string(canonical, binding.binding_id.as_str());
    push_string(canonical, binding.source.host_id.as_str());
    push_string(canonical, binding.source.boot_id.as_str());
    push_string(canonical, binding.source.endpoint_id.as_str());
    push_string(canonical, binding.sink.host_id.as_str());
    push_string(canonical, binding.sink.boot_id.as_str());
    push_string(canonical, binding.sink.endpoint_id.as_str());
    push_string(canonical, binding.base.as_str());
    push_string(canonical, binding.base_instance_id.as_str());
    match &binding.credential {
        LinkCredentialReference::None => canonical.push(0),
        LinkCredentialReference::Opaque(reference) => {
            canonical.push(1);
            push_string(canonical, reference.as_str());
        }
    }
    match &binding.authority {
        LinkAuthorityReference::ProcessOwned => canonical.push(0),
        LinkAuthorityReference::Grant(grant_id) => {
            canonical.push(1);
            push_string(canonical, grant_id.as_str());
        }
    }
    canonical.extend_from_slice(&binding.limits.maximum_in_flight_items.to_le_bytes());
    push_u32(canonical, binding.limits.maximum_payload_bytes);
    push_u32(canonical, binding.limits.maximum_buffered_bytes);
    push_u32(canonical, binding.limits.maximum_frame_bytes);
}

fn push_admitted_line(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    line: &AdmittedLine,
) {
    push_string(canonical, line.line_id.as_str());
    push_bound_link(canonical, &line.binding);
    canonical.push(line.contract.scope as u8);
    canonical.push(line.contract.traffic_shape as u8);
    canonical.push(line.contract.duplex as u8);
    canonical.push(line.contract.ordering as u8);
    canonical.push(line.contract.reliability as u8);
    canonical.push(line.contract.continuation as u8);
    canonical.push(line.contract.security as u8);
}

/// Canonical identity encoding only; does not validate commitments or grant authority.
pub fn compute_plan_id(
    plot_identity: &PlotIdentity,
    realization_backs: &[PlotBack],
    activations: &[PlannedActivationEntry],
    commitments: &[FragmentCommitment],
    activation_preparations: &[PlannedActivationPreparationBinding],
) -> PlanId {
    root::compute_plan_fingerprint_ref(root::PlanFingerprintInputRef {
        source_document_id: plot_identity.source_document_id.as_str(),
        checked_plot_id: plot_identity.checked_plot_id.as_str(),
        expanded_plot_id: plot_identity.expanded_plot_id.as_str(),
        realization_backs,
        activations,
        commitments: root::FragmentCommitmentRefs::Owned(commitments),
        activation_preparations,
    })
}

fn push_activation_front(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    front: &crate::PlannedActivationFront,
) {
    push_string(canonical, front.front_port_id.as_str());
    push_string(canonical, front.value_kind.as_str());
    push_optional_string(
        canonical,
        front.abnormal_kind.as_ref().map(|kind| kind.as_str()),
    );
}

fn push_ports(canonical: &mut impl CanonicalSink, ports: &[PortDescriptor]) {
    push_ports_ref(canonical, crate::PortDescriptorRefs::Owned(ports));
}
fn push_ports_ref(canonical: &mut impl CanonicalSink, ports: crate::PortDescriptorRefs<'_>) {
    push_u32(canonical, ports.len() as u32);
    for port in ports.iter() {
        push_string(canonical, port.port_id);
        push_string(canonical, port.value_kind);
        canonical.push(match port.direction {
            PortDirection::Input => 0,
            PortDirection::Output => 1,
        });
        canonical.push(match port.temporal {
            PortTemporal::Value => 0,
            PortTemporal::Flow { closes: false } => 1,
            PortTemporal::Flow { closes: true } => 2,
            PortTemporal::Current => 3,
        });
        match port.abnormal_kind {
            Some(kind) => {
                canonical.push(1);
                push_string(canonical, kind);
            }
            None => canonical.push(0),
        }
    }
}

mod configuration;
pub(crate) use configuration::{configuration_fingerprint, push_configuration};
