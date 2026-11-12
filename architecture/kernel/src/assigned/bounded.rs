//! The existing active-prefix scheduler's tables inside fixed storage capacity.
use super::*;

/// Exact checked topology prefix, with inert capacity outside its actual counts.
/// This is table preparation, not resource admission, Back possession or start.
pub struct BoundedAssignedKernelTables<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
> {
    identity: c::AssignedPlanView,
    pub nodes: [NodeSpec<P>; N],
    pub cords: [CordSpec; C],
    pub routes: k::FixedRoutes<R, T>,
    pub calls: k::FixedHostCallBindings<H>,
    startup_order: [k::NodeId; N],
}
impl<
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    > BoundedAssignedKernelTables<N, C, P, R, T, H>
{
    /// Use only with the scheduler's explicit active-count constructor. The
    /// separately pinned layout and ordinary Core validation remain mandatory.
    #[inline(always)] // Fuse the owned table-field transfer with the caller's destination.
    pub fn prepare_with_fore(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        fore: &[c::AssignedForeRequirement],
    ) -> Result<Self, AssignedKernelRefusal> {
        let tables = AssignedKernelTables::prepare_capacity_profile(
            RecordInput::Bytes(bytes),
            maxima,
            required,
            computations,
            layout,
            Some(fore),
            false,
        )?;
        Ok(Self {
            identity: tables.identity,
            nodes: tables.nodes,
            cords: tables.cords,
            routes: tables.routes,
            calls: tables.calls,
            startup_order: tables.startup_order,
        })
    }

    pub fn identity(&self) -> c::AssignedPlanView {
        self.identity
    }
    pub fn active_nodes(&self) -> usize {
        usize::from(self.identity.counts[0])
    }
    pub fn active_cords(&self) -> usize {
        usize::from(self.identity.counts[3])
    }
    pub fn startup_order(&self) -> &[k::NodeId] {
        &self.startup_order[..self.active_nodes()]
    }

    /// Visit only the actual dependency order; unused slots stay None. All
    /// workset checks/reservations must precede any provider transfer here.
    pub fn prepare_backs<B, E>(
        &self,
        mut prepare: impl FnMut(k::NodeId) -> Result<B, E>,
    ) -> Result<[Option<B>; N], E> {
        let mut backs = core::array::from_fn(|_| None);
        for node in self.startup_order() {
            backs[usize::from(node.0)] = Some(prepare(*node)?);
        }
        Ok(backs)
    }
}
