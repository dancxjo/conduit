//! Capacity observation inside the owner of CheckedFront's private Vec fields.
use super::*;
use crate::metadata_backing::Bytes;
impl CheckedFront {
    /// Reachable owned payload capacities, excluding inline Self, allocator
    /// overhead and construction/validation scratch. Shared nested contract
    /// backing is conservatively counted per owner, never an exclusive loan.
    /// This neither checks a Fore nor reserves any memory or authority.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let CheckedFront {
            startup_parameters,
            inputs,
            outputs,
            shorthand,
            resource_ports,
            value_contracts,
        } = self;
        let mut n = Bytes::default();
        n.startup(startup_parameters)?;
        n.ports(inputs)?;
        n.ports(outputs)?;
        n.shorthand(shorthand)?;
        n.resources(resource_ports)?;
        n.values(value_contracts)?;
        Some(n.0)
    }
}
