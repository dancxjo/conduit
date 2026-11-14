//! Explicit schema-8 construction; numeric tables create no live Line authority.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum ForeProfile<'a> {
    Local(&'a [c::AssignedForeRequirement]),
    Line(c::AssignedLineForeProfile<'a>),
}
impl ForeProfile<'_> {
    pub(super) fn check<
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    >(
        self,
        bytes: &[u8],
        profile: &b::AssignedBodyPreparationProfile<'_>,
        layout: &ResidentLayout<'_>,
    ) -> Result<(), conduit_kernel::assigned::AssignedKernelRefusal> {
        match self {
            Self::Local(fore) => {
                BoundedAssignedKernelTables::<N, C, P, R, T, H>::prepare_with_fore(
                    bytes,
                    profile.maxima,
                    profile.requirements,
                    profile.computations,
                    layout,
                    fore,
                )
            }
            Self::Line(fore) => {
                BoundedAssignedKernelTables::<N, C, P, R, T, H>::prepare_with_line_fore(
                    bytes,
                    profile.maxima,
                    profile.requirements,
                    profile.computations,
                    layout,
                    fore,
                )
            }
        }
        .map(|_| ())
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn write<
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    >(
        self,
        bytes: &[u8],
        profile: &b::AssignedBodyPreparationProfile<'_>,
        layout: &ResidentLayout<'_>,
        nodes: &mut [NodeSpec<P>; N],
        cords: &mut [CordSpec; C],
        routes: &mut FixedRoutes<R, T>,
        calls: &mut FixedHostCallBindings<H>,
    ) -> Result<c::AssignedPlanView, conduit_kernel::assigned::AssignedKernelRefusal> {
        match self {
            Self::Local(fore) => BoundedAssignedKernelTables::<N, C, P, R, T, H>::write_with_fore(
                bytes,
                profile.maxima,
                profile.requirements,
                profile.computations,
                layout,
                fore,
                nodes,
                cords,
                routes,
                calls,
            ),
            Self::Line(fore) => {
                BoundedAssignedKernelTables::<N, C, P, R, T, H>::write_with_line_fore(
                    bytes,
                    profile.maxima,
                    profile.requirements,
                    profile.computations,
                    layout,
                    fore,
                    nodes,
                    cords,
                    routes,
                    calls,
                )
            }
        }
    }
}
impl<
        B: PreparedBacks<N, P>,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const S: usize,
        L: ExecutionSigns,
        const K: usize,
        V: ExecutionValues,
        const Q: usize,
        const M: usize,
    > PreparedFragment<B, N, C, P, R, T, H, S, L, K, V, Q, M>
{
    /// Explicit schema-8 producer factory entrance with independently pinned
    /// Line/Fore and borrowed immutable terminal laws. Exact table/layout checks
    /// precede the arbitrary factory, then the SAME actual terminal report,
    /// allocation/funding and native Host Call checks apply as the local path.
    /// Root must separately retain carrier/session possession and authority;
    /// this preparation ticket has only the existing whole-workset admit/Start.
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    pub fn prepare_with_line_fore_terminal_transductions_ref<E>(
        approved: &b::DelegatedBodyFragment<'_>,
        profile: b::AssignedBodyLineForePreparationProfile<'_>,
        layout: &ResidentLayout<'_>,
        values: V,
        signs: L,
        maximum_retained_bytes: usize,
        factory: impl FnOnce() -> Result<B, E>,
        expected: &[[Option<AssignedTerminalTransduction>; P]; N],
    ) -> Result<Self, Refusal<E>> {
        if profile.preparation.fore.is_some() {
            return Err(Refusal::Scope);
        }
        Self::prepare_inner(
            approved,
            profile.preparation,
            ForeProfile::Line(profile.line_fore),
            layout,
            values,
            signs,
            maximum_retained_bytes,
            factory,
            Some(ExpectedTerminalTable::Inline(expected)),
        )
    }
}
