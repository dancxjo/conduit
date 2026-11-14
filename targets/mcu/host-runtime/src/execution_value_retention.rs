//! Actual prepared-owner evidence, never a grant or a complete pool certificate.
/// Extra input references held outside Cord queues between committed Steps.
/// `possible_ports` are alternatives sharing one total `maximum_values` bound;
/// it is not a separate bound for each port. Old held state can coexist with a
/// queued replacement and transient output, which must be funded independently.
/// Copied private bytes do not hold pool references and are charged as backing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedInputs<const P: usize> {
    /// No audited lifetime bound. A compact-stock calculation must refuse or
    /// keep the original dense representation, rather than assume zero.
    Unknown,
    Bounded {
        maximum_values: u16,
        possible_ports: [bool; P],
    },
}
impl<const P: usize> RetainedInputs<P> {
    pub const NONE: Self = Self::Bounded {
        maximum_values: 0,
        possible_ports: [false; P],
    };
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use crate::execution_software::SoftwareBack;
    use alloc::boxed::Box;
    use conduit_core::{kind_id, CheckedValueContract, StructuredInfoType};

    #[test]
    fn actual_software_retention_keeps_state_and_unknown_owners_distinct() {
        let schema = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
        let contract = CheckedValueContract {
            value_kind: kind_id("value/u8"),
            maximum_bytes: 1,
            constraints: alloc::vec![],
        };
        let state = SoftwareBack::SeededState(Box::new(
            conduit_composite::SeededStateBack::prepare(&contract, &schema).unwrap(),
        ));
        assert_eq!(
            state.retained_inputs::<3>(),
            RetainedInputs::Bounded {
                maximum_values: 1,
                possible_ports: [true, true, false],
            }
        );
        // A report cannot silently truncate the state replacement port.
        assert_eq!(state.retained_inputs::<1>(), RetainedInputs::Unknown);
        let material = Box::leak(schema.canonical_bytes().unwrap().into_boxed_slice());
        let value = Box::leak(
            conduit_core::StructuredInfoValue::leaf(schema, alloc::vec![7])
                .unwrap()
                .canonical_bytes()
                .unwrap()
                .into_boxed_slice(),
        );
        let linked = SoftwareBack::linked_source_static(material, value, value.len()).unwrap();
        assert_eq!(linked.retained_inputs::<2>(), RetainedInputs::NONE);
        let wait = conduit_kernel::ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 8,
        };
        let tick = SoftwareBack::tick(wait, 2).unwrap();
        assert_eq!(tick.retained_inputs::<2>(), RetainedInputs::Unknown);
    }
}
