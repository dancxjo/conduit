//! Strict assigned-record decoding into an existing inert table destination.
use super::*;
impl<
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    > AssignedKernelTables<N, C, P, R, T, H>
{
    /// Same strict exact-count, Core, layout and Fore checks as
    /// `prepare_records_with_fore`, writing into caller-owned initialized tables.
    /// Refusal may leave a partial destination; it must not be executed.
    /// This grants no admission, provider possession or authority to start.
    #[allow(clippy::too_many_arguments)]
    pub fn write_records_with_fore(
        header: &[u8],
        records: &[c::AssignedRecord<'_>],
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
        Self::write_capacity_profile(
            RecordInput::Records { header, records },
            maxima,
            required,
            computations,
            layout,
            Some(fore),
            true,
            nodes,
            cords,
            routes,
            calls,
            &mut startup_order,
        )
    }
}
