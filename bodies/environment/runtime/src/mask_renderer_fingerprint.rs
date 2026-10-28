//! Comparison-only provenance minted by checked renderer preparation. These
//! digests carry no Core capability, physical owner or execution permission.
use super::*;
use sha2::{Digest, Sha256};
struct Transcript(Sha256);
impl Transcript {
    fn new(domain: &[u8]) -> Self {
        Self(Sha256::new_with_prefix(domain))
    }
    fn text(&mut self, value: &str) {
        self.0.update((value.len() as u64).to_le_bytes());
        self.0.update(value.as_bytes());
    }
    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}
pub(super) fn owner(identity: &RendererIdentity, bounds: Bounds, node: NodeId) -> [u8; 32] {
    // Exhaustive destructuring makes future identity fields a compile error
    // until their comparison meaning is explicitly included.
    let RendererIdentity {
        host_id,
        boot_id,
        base_id,
        provider_instance_id,
        provider_generation,
        resource_pool_id,
        resource_generation_id,
        envelope_id,
        artifact_id,
        implementation_id,
        base_implementation_id,
    } = identity;
    let Bounds {
        face_bytes,
        show_bytes,
        resource_bytes,
    } = bounds;
    let mut h = Transcript::new(b"conduit.environment/checked-renderer-owner@1\0");
    for value in [
        host_id.as_str(),
        boot_id.as_str(),
        base_id.as_str(),
        provider_instance_id.as_str(),
        resource_pool_id.as_str(),
        resource_generation_id.0.as_str(),
        envelope_id.as_str(),
        artifact_id.as_str(),
        implementation_id.as_str(),
        base_implementation_id.as_str(),
    ] {
        h.text(value);
    }
    h.0.update(provider_generation.to_le_bytes());
    h.0.update(face_bytes.to_le_bytes());
    h.0.update(show_bytes.to_le_bytes());
    h.0.update(resource_bytes.to_le_bytes());
    h.0.update(node.0.to_le_bytes());
    h.finish()
}
/// All fields of the original same_scope comparison. Only the independently
/// issued child's active_play_id changes, and is checked against its actual Play.
pub(super) fn scope(scope: &c::BaseCapabilityScope) -> [u8; 32] {
    let c::BaseCapabilityScope {
        host_id,
        boot_id,
        base_instance_id,
        base_provider_generation,
        plan_id,
        authority_grant_id,
        authority_contract_id,
        capability_id,
        implementation_id,
        operation_contract_id,
        subject_kind,
        resource_pool_id,
        resource_generation_id,
        envelope_id,
        maximum_parameter_bytes,
        maximum_result_bytes,
        maximum_work_units,
        maximum_in_flight,
        maximum_operations,
        active_play_id: _,
    } = scope;
    let mut h = Transcript::new(b"conduit.environment/checked-renderer-scope@1\0");
    for value in [
        host_id.as_str(),
        boot_id.as_str(),
        base_instance_id.as_str(),
        plan_id.as_str(),
        authority_grant_id.as_str(),
        authority_contract_id.as_str(),
        capability_id.as_str(),
        implementation_id.as_str(),
        operation_contract_id.as_str(),
        subject_kind.as_str(),
        resource_pool_id.as_str(),
        resource_generation_id.0.as_str(),
        envelope_id.as_str(),
    ] {
        h.text(value);
    }
    h.0.update(base_provider_generation.to_le_bytes());
    h.0.update(maximum_parameter_bytes.to_le_bytes());
    h.0.update(maximum_result_bytes.to_le_bytes());
    h.0.update(maximum_work_units.to_le_bytes());
    h.0.update(maximum_in_flight.to_le_bytes());
    h.0.update(maximum_operations.to_le_bytes());
    h.finish()
}
pub(super) fn parent(active: &c::ActivePlayIdentity) -> [u8; 32] {
    let mut h = Transcript::new(b"conduit.environment/checked-renderer-parent@1\0");
    for value in [
        active.plan_id.as_str(),
        active.host_id.as_str(),
        active.boot_id.as_str(),
    ] {
        h.text(value);
    }
    h.finish()
}
pub(super) fn active(active: &c::ActivePlayIdentity) -> [u8; 32] {
    let mut h = Transcript::new(b"conduit.environment/checked-renderer-active@1\0");
    for value in [
        active.plan_id.as_str(),
        active.host_id.as_str(),
        active.boot_id.as_str(),
        active.active_play_id.as_str(),
    ] {
        h.text(value);
    }
    h.0.update(active.play_sequence.to_le_bytes());
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> RendererIdentity {
        RendererIdentity {
            host_id: "host/1".into(),
            boot_id: "boot/1".into(),
            base_id: "base/1".into(),
            provider_instance_id: "instance/1".into(),
            provider_generation: 1,
            resource_pool_id: "pool/1".into(),
            resource_generation_id: c::ResourceGenerationId("generation/1".into()),
            envelope_id: "envelope/1".into(),
            artifact_id: "artifact/1".into(),
            implementation_id: "implementation/1".into(),
            base_implementation_id: "base-implementation/1".into(),
        }
    }
    fn bounds() -> Bounds {
        Bounds {
            face_bytes: 8192,
            show_bytes: 8192,
            resource_bytes: 24,
        }
    }
    fn scope_value() -> c::BaseCapabilityScope {
        c::BaseCapabilityScope {
            host_id: "host/1".into(),
            boot_id: "boot/1".into(),
            base_instance_id: "instance/1".into(),
            base_provider_generation: 1,
            plan_id: "plan/1".into(),
            active_play_id: "play/1".into(),
            authority_grant_id: "grant/1".into(),
            authority_contract_id: "authority/1".into(),
            capability_id: "capability/1".into(),
            implementation_id: "implementation/1".into(),
            operation_contract_id: "operation/1".into(),
            subject_kind: "kind/1".into(),
            resource_pool_id: "pool/1".into(),
            resource_generation_id: c::ResourceGenerationId("generation/1".into()),
            envelope_id: "envelope/1".into(),
            maximum_parameter_bytes: 8192,
            maximum_result_bytes: 8192,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 2,
        }
    }
    #[test]
    fn every_owner_identity_bounds_and_node_field_changes_comparison() {
        let original = identity();
        let bounds = bounds();
        let expected = owner(&original, bounds, NodeId(2));
        macro_rules! text {
            ($field:ident) => {{
                let mut next = original.clone();
                next.$field = "foreign".into();
                assert_ne!(
                    owner(&next, bounds, NodeId(2)),
                    expected,
                    stringify!($field)
                );
            }};
        }
        text!(host_id);
        text!(boot_id);
        text!(base_id);
        text!(provider_instance_id);
        text!(resource_pool_id);
        text!(envelope_id);
        text!(artifact_id);
        text!(implementation_id);
        text!(base_implementation_id);
        let mut next = original.clone();
        next.provider_generation += 1;
        assert_ne!(owner(&next, bounds, NodeId(2)), expected);
        let mut next = original.clone();
        next.resource_generation_id.0.push('x');
        assert_ne!(owner(&next, bounds, NodeId(2)), expected);
        for field in 0..3 {
            let mut changed = bounds;
            match field {
                0 => changed.face_bytes -= 1,
                1 => changed.show_bytes -= 1,
                _ => changed.resource_bytes += 1,
            };
            assert_ne!(owner(&original, changed, NodeId(2)), expected);
        }
        assert_ne!(owner(&original, bounds, NodeId(3)), expected);
    }
    #[test]
    fn every_stable_scope_field_changes_comparison_and_only_active_id_is_excluded() {
        let original = scope_value();
        let expected = scope(&original);
        macro_rules! text {
            ($field:ident) => {{
                let mut next = original.clone();
                next.$field = "foreign".into();
                assert_ne!(scope(&next), expected, stringify!($field));
            }};
        }
        text!(host_id);
        text!(boot_id);
        text!(base_instance_id);
        text!(plan_id);
        text!(authority_grant_id);
        text!(authority_contract_id);
        text!(capability_id);
        text!(implementation_id);
        text!(operation_contract_id);
        text!(subject_kind);
        text!(resource_pool_id);
        text!(envelope_id);
        let mut next = original.clone();
        next.resource_generation_id.0.push('x');
        assert_ne!(scope(&next), expected);
        for field in 0..6 {
            let mut next = original.clone();
            match field {
                0 => next.base_provider_generation += 1,
                1 => next.maximum_parameter_bytes -= 1,
                2 => next.maximum_result_bytes -= 1,
                3 => next.maximum_work_units += 1,
                4 => next.maximum_in_flight += 1,
                _ => next.maximum_operations += 1,
            };
            assert_ne!(scope(&next), expected);
        }
        let mut next = original.clone();
        next.active_play_id = "play/next".into();
        assert_eq!(scope(&next), expected);
    }
    #[test]
    fn parent_and_exact_active_transcripts_are_framed_and_distinct() {
        let original = c::bind_active_play(&"plan/1".into(), &"host/1".into(), &"boot/1".into(), 1);
        for field in 0..5 {
            let mut next = original.clone();
            match field {
                0 => next.plan_id = "foreign".into(),
                1 => next.host_id = "foreign".into(),
                2 => next.boot_id = "foreign".into(),
                3 => next.active_play_id = "foreign".into(),
                _ => next.play_sequence += 1,
            };
            assert_ne!(active(&next), active(&original));
            if field < 3 {
                assert_ne!(parent(&next), parent(&original));
            } else {
                assert_eq!(parent(&next), parent(&original));
            }
        }
        let mut a = original.clone();
        a.host_id = "ab".into();
        a.boot_id = "c".into();
        let mut b = original.clone();
        b.host_id = "a".into();
        b.boot_id = "bc".into();
        assert_ne!(parent(&a), parent(&b));
        assert_ne!(parent(&original), active(&original));
    }
}
