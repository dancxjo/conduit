//! Exact installed neutral entry. No names or hashes select protocol behavior.
#[allow(dead_code)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/workset.rs"));
}
pub use generated::{entry_0 as source, entry_0_layout as layout};
const _: () = assert!(generated::ENTRY_COUNT == 1);
#[allow(dead_code)]
pub mod native_images {
    include!(concat!(env!("OUT_DIR"), "/native_images.rs"));
}
use conduit_core as c;
pub const N: usize = layout::COUNTS[(c::ASSIGNED_NODE - 1) as usize] as usize;
pub const C: usize = layout::COUNTS[(c::ASSIGNED_CORD - 1) as usize] as usize;
/// Actual independently lowered Cord item slots. Cord count is not queue depth.
pub const Q: usize = layout::CORD_VALUE_SLOT_CAPACITIES.len();
pub const P: usize = layout::PORTS_PER_NODE;
pub const R: usize = N * P;
pub const T: usize = layout::COUNTS[(c::ASSIGNED_ROUTE_TARGET - 1) as usize] as usize;
pub const H: usize = layout::HOST_CALL_SLOTS;
pub const K: usize = layout::PENDING_HOST_CALL_SLOTS;
pub const S: usize = layout::SIGN_ITEMS;
pub const FORE_PORTS: usize = layout::FORE_PORTS;
const fn narrow<const W: usize>(
    source: &[[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; W]; N],
) -> [[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]; N] {
    assert!(P <= W);
    let mut result = [[None; P]; N];
    let mut n = 0;
    while n < N {
        let mut p = 0;
        while p < W {
            if p < P {
                result[n][p] = source[n][p];
            } else {
                assert!(source[n][p].is_none());
            }
            p += 1;
        }
        n += 1;
    }
    result
}
pub static TERMINALS: [[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]; N] =
    narrow(&layout::GENERATED_TERMINAL_TRANSDUCTIONS);
pub fn profile<'a>(
    actual: &'a c::HostAdvertisement,
    fore: &'a [c::AssignedForeRequirement],
) -> conduit_body::AssignedBodyPreparationProfile<'a> {
    conduit_body::AssignedBodyPreparationProfile {
        maxima: layout::MAXIMA,
        requirements: c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(actual.host_id.as_str()),
            boot: c::AssignedIdentity::from_text(actual.boot_id.as_str()),
            host_calls: layout::HOST_CALL_CONTRACTS,
            resources: layout::RESOURCE_ORDINALS,
            remote_bindings: &[],
        },
        computations: c::AssignedComputationProfile {
            maximum_text_bytes: layout::COMPUTATION_MAXIMUM_TEXT_BYTES,
            maximum_total_text_bytes: layout::COMPUTATION_MAXIMUM_TOTAL_TEXT_BYTES,
            requirements: layout::COMPUTATIONS,
        },
        fore: Some(fore),
    }
}
