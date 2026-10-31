//! Finite small-Kind uniqueness scratch; large Kinds retain the original tree.
use crate::PortDescriptorRefs;
use alloc::collections::BTreeSet;
const SMALL_PORTS: usize = 16;
// Keep temporary synchronous scratch out of retained callers/async frames.
#[inline(never)]
pub(super) fn has_duplicate(
    inputs: PortDescriptorRefs<'_>,
    outputs: PortDescriptorRefs<'_>,
) -> bool {
    if inputs
        .len()
        .checked_add(outputs.len())
        .is_some_and(|n| n <= SMALL_PORTS)
    {
        let mut previous = [""; SMALL_PORTS];
        for (index, port) in inputs.iter().chain(outputs.iter()).enumerate() {
            if previous[..index].contains(&port.port_id) {
                return true;
            }
            previous[index] = port.port_id;
        }
        false
    } else {
        // Preserve the exact original algorithm/ordering/complexity above16.
        // This fallback remains allocation-dependent; it is not an OOM proof.
        let mut symbols = BTreeSet::new();
        inputs
            .iter()
            .chain(outputs.iter())
            .any(|port| !symbols.insert(port.port_id))
    }
}
#[cfg(test)]
mod tests {
    use crate::*;
    use alloc::{format, vec::Vec};
    fn kind(count: usize) -> crate::capability_offer::Kind {
        let ports: Vec<_> = (0..count)
            .map(|index| PortDescriptor {
                port_id: format!("port/{index}").into(),
                value_kind: "u64".into(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            })
            .collect();
        let split = count / 2;
        let mut outputs = ports[split..].to_vec();
        for port in &mut outputs {
            port.direction = PortDirection::Output;
        }
        crate::capability_offer::Kind {
            startup_parameters: alloc::vec![],
            shorthand: None,
            kind_id: "kind/port-symbols".into(),
            kind_contract_revision: "revision/1".into(),
            inputs: ports[..split].to_vec(),
            outputs,
            configuration: alloc::vec![],
            semantic_laws: alloc::vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        }
    }
    #[test]
    fn bounded_fast_path_and_large_fallback_match_the_frozen_full_validator() {
        for count in [0, 1, 2, 16, 17, 64] {
            let original = kind(count);
            // Kind::validate additionally checks the frozen pre-view oracle in
            // this owning Core unit harness, including exact error precedence.
            assert_eq!(original.validate(), Ok(()));
            if count >= 2 {
                let mut duplicate = original.clone();
                duplicate.outputs[0].port_id = duplicate.inputs[0].port_id.clone();
                assert_eq!(
                    duplicate.validate(),
                    Err(crate::KindValidationError::DuplicatePortSymbol)
                );
                duplicate.kind_id = "".into();
                assert_eq!(
                    duplicate.validate(),
                    Err(crate::KindValidationError::EmptyId)
                );
            }
            if count >= 1 {
                let mut empty = original;
                empty.outputs[0].port_id = "".into();
                assert_eq!(empty.validate(), Ok(()));
                if count >= 2 {
                    empty.inputs[0].port_id = "".into();
                    assert_eq!(
                        empty.validate(),
                        Err(crate::KindValidationError::DuplicatePortSymbol)
                    );
                    empty.outputs[0].abnormal_kind = Some("".into());
                    assert_eq!(
                        empty.validate(),
                        Err(crate::KindValidationError::EmptyAbnormalTerminalKind)
                    );
                }
            }
        }
    }
}
