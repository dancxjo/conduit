//! Exact fixed-storage profile selected before Plan lowering and Play start.

/// Backing width of the allocation-independent kernel tables emitted by this
/// package. This is a storage-profile fact, not a semantic or planner limit.
pub const FIXED_KERNEL_STORAGE_PORTS_PER_NODE: usize = 16;

/// Exact fixed-storage limits selected by a Host before lowering and Play.
///
/// A constrained Host may select a narrower profile than the backing tables;
/// lowering then refuses a wider Plan before any kernel state is created.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct KernelStorageProfile {
    maximum_ports_per_node: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KernelStorageProfileError {
    ZeroPortsPerNode,
    ExceedsFixedStorage { requested: usize, available: usize },
}

impl KernelStorageProfile {
    pub const fn new(maximum_ports_per_node: usize) -> Result<Self, KernelStorageProfileError> {
        if maximum_ports_per_node == 0 {
            return Err(KernelStorageProfileError::ZeroPortsPerNode);
        }
        if maximum_ports_per_node > FIXED_KERNEL_STORAGE_PORTS_PER_NODE {
            return Err(KernelStorageProfileError::ExceedsFixedStorage {
                requested: maximum_ports_per_node,
                available: FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            });
        }
        Ok(Self {
            maximum_ports_per_node,
        })
    }

    pub const fn maximum_ports_per_node(self) -> usize {
        self.maximum_ports_per_node
    }
}

pub const FIXED_KERNEL_STORAGE_PROFILE: KernelStorageProfile = KernelStorageProfile {
    maximum_ports_per_node: FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_are_finite_and_cannot_outgrow_fixed_backing() {
        assert_eq!(
            KernelStorageProfile::new(0),
            Err(KernelStorageProfileError::ZeroPortsPerNode)
        );
        assert_eq!(
            KernelStorageProfile::new(FIXED_KERNEL_STORAGE_PORTS_PER_NODE + 1),
            Err(KernelStorageProfileError::ExceedsFixedStorage {
                requested: FIXED_KERNEL_STORAGE_PORTS_PER_NODE + 1,
                available: FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            })
        );
        assert_eq!(
            KernelStorageProfile::new(1)
                .unwrap()
                .maximum_ports_per_node(),
            1
        );
    }
}
