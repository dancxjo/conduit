//! Work-class identity comes from checked semantic meaning, never a producer's
//! opaque rating. Dynamic work extent and hard eligibility remain separate.
use super::{CostEvidenceRefusal as Refusal, CostEvidenceSource};
use conduit_plot::CheckedGear;

/// Borrowed checked meaning, including exact startup configuration and typed
/// ports. Two Gear instances may share this class only when their entire checked
/// meaning agrees; the instance Gear ID is deliberately excluded.
///
/// This is not a workload-size estimate: equal classes can receive different
/// runtime inputs. A service forecast must also match its admitted work extent.
#[derive(Debug, Clone, Copy)]
pub struct CostWorkClass<'a> {
    gear: &'a CheckedGear,
}
impl<'a> CostWorkClass<'a> {
    pub fn from_checked_gear(gear: &'a CheckedGear) -> Self {
        Self { gear }
    }
    pub const fn gear(self) -> &'a CheckedGear {
        self.gear
    }
    pub fn validate_demand(self, demand: &CheckedGear) -> Result<(), Refusal> {
        if same_meaning(self.gear, demand) {
            Ok(())
        } else {
            Err(Refusal::WorkClassChanged)
        }
    }
    /// Reuses the ordinary Fore and Kind-law match. This checks semantic fit,
    /// not authority, resource possession, queue capacity or current health.
    pub fn validate_source(self, source: &CostEvidenceSource<'_>) -> Result<(), Refusal> {
        if self.gear.accepts_realization(source.offer()) {
            Ok(())
        } else {
            Err(Refusal::IncompatibleRealization)
        }
    }
}
fn same_meaning(left: &CheckedGear, right: &CheckedGear) -> bool {
    // Exhaustive destructuring makes a newly added checked field a compile-time
    // obligation to decide whether it affects this cost-class identity.
    let CheckedGear {
        gear_id: _,
        kind_id,
        kind_contract_revision,
        startup_parameters,
        shorthand,
        inputs,
        outputs,
        semantic_contract,
        terminal_transductions,
        resource_ports,
        configuration,
        pool_references,
    } = left;
    kind_id == &right.kind_id
        && kind_contract_revision == &right.kind_contract_revision
        && startup_parameters == &right.startup_parameters
        && shorthand == &right.shorthand
        && inputs == &right.inputs
        && outputs == &right.outputs
        && semantic_contract == &right.semantic_contract
        && terminal_transductions == &right.terminal_transductions
        && resource_ports == &right.resource_ports
        && configuration == &right.configuration
        && pool_references == &right.pool_references
}
