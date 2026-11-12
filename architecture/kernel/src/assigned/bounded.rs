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
            Some(ForeProfile::Local(fore)),
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

    /// Decode into caller-owned initialized inert tables. This supplies no
    /// admission, provider possession, or execution authority. On refusal the
    /// destination may be partially written and must not be executed.
    #[allow(clippy::too_many_arguments)]
    pub fn write_with_fore(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        fore: &[c::AssignedForeRequirement],
        nodes: &mut [NodeSpec<P>; N],
        cords: &mut [CordSpec; C],
        routes: &mut k::FixedRoutes<R, T>,
        calls: &mut k::FixedHostCallBindings<H>,
    ) -> Result<c::AssignedPlanView, AssignedKernelRefusal> {
        let mut startup_order = [k::NodeId(u16::MAX); N];
        AssignedKernelTables::write_capacity_profile(
            RecordInput::Bytes(bytes),
            maxima,
            required,
            computations,
            layout,
            Some(ForeProfile::Local(fore)),
            false,
            nodes,
            cords,
            routes,
            calls,
            &mut startup_order,
        )
    }

    /// Explicit schema-8 active-prefix preparation; storage capacities remain
    /// unchanged. Root still owns all live Line/session and Start admission.
    pub fn prepare_with_line_fore(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        profile: c::AssignedLineForeProfile<'_>,
    ) -> Result<Self, AssignedKernelRefusal> {
        let tables = AssignedKernelTables::prepare_capacity_profile(
            RecordInput::Bytes(bytes),
            maxima,
            required,
            computations,
            layout,
            Some(ForeProfile::Line(profile)),
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

    /// Same schema-8 checks, writing only inert caller-owned active-prefix
    /// tables. Partial destinations on refusal must not be executed.
    #[allow(clippy::too_many_arguments)]
    pub fn write_with_line_fore(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        profile: c::AssignedLineForeProfile<'_>,
        nodes: &mut [NodeSpec<P>; N],
        cords: &mut [CordSpec; C],
        routes: &mut k::FixedRoutes<R, T>,
        calls: &mut k::FixedHostCallBindings<H>,
    ) -> Result<c::AssignedPlanView, AssignedKernelRefusal> {
        let mut startup_order = [k::NodeId(u16::MAX); N];
        AssignedKernelTables::write_capacity_profile(
            RecordInput::Bytes(bytes),
            maxima,
            required,
            computations,
            layout,
            Some(ForeProfile::Line(profile)),
            false,
            nodes,
            cords,
            routes,
            calls,
            &mut startup_order,
        )
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
