//! Opt-in schema-8 table preparation, not live Line or session admission.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum ForeProfile<'a> {
    Local(&'a [c::AssignedForeRequirement]),
    Line(c::AssignedLineForeProfile<'a>),
}
impl<'a> ForeProfile<'a> {
    pub(super) fn local(self) -> &'a [c::AssignedForeRequirement] {
        match self {
            Self::Local(fore) => fore,
            Self::Line(profile) => profile.local,
        }
    }
    fn lines(self) -> &'a [c::AssignedLineForeRequirement] {
        match self {
            Self::Local(_) => &[],
            Self::Line(profile) => profile.lines,
        }
    }
    pub(super) fn len(self) -> usize {
        self.local().len() + self.lines().len()
    }
    pub(super) fn matches(self, endpoint: u16, cord: u16, source: bool) -> bool {
        self.local()
            .iter()
            .chain(self.lines().iter().map(|line| line.fore()))
            .any(|fore| {
                fore.endpoint() == endpoint
                    && fore.cord() == cord
                    && (fore.direction() == c::AssignedForeDirection::Input) == source
            })
    }
    pub(super) fn pin(self, cords: &mut [CordSpec]) -> Result<(), AssignedKernelRefusal> {
        // Local variants alone receive the existing descriptive local custody.
        // Selected Lines retain ordinary remote delivery and Sign obligations.
        local_fore::pin(cords, self.local())?;
        for port in self.lines().iter().map(|line| line.fore()) {
            let cord = cords
                .get(usize::from(port.cord()))
                .ok_or(AssignedKernelRefusal::Layout)?;
            let endpoint = k::CordEndpoint::Remote(k::RemoteEndpointId(port.endpoint()));
            let matches = match port.direction() {
                c::AssignedForeDirection::Input => {
                    cord.source == endpoint && matches!(cord.sink, k::CordEndpoint::Local { .. })
                }
                c::AssignedForeDirection::Output => {
                    cord.sink == endpoint && matches!(cord.source, k::CordEndpoint::Local { .. })
                }
            };
            if !matches {
                return Err(AssignedKernelRefusal::Layout);
            }
        }
        Ok(())
    }
}

impl<
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    > AssignedKernelTables<N, C, P, R, T, H>
{
    /// Consume schema-8 representation through its exact trusted Root profile.
    /// Root must retain typed ABI/Line/session commitments and separately check
    /// Body/Plan/Play containment, actual carrier possession and live admission.
    /// This does not support schema-8 borrowed-record preparation.
    pub fn prepare_with_line_fore(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        profile: c::AssignedLineForeProfile<'_>,
    ) -> Result<Self, AssignedKernelRefusal> {
        Self::prepare_profile(
            RecordInput::Bytes(bytes),
            maxima,
            required,
            computations,
            layout,
            Some(ForeProfile::Line(profile)),
        )
    }

    /// Write exact schema-8 tables into existing inert caller-owned storage.
    /// Refusal can partially write the destination, which must not be executed.
    /// Numeric remote endpoints confer no Line/session or equipment authority.
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
        Self::write_capacity_profile(
            RecordInput::Bytes(bytes),
            maxima,
            required,
            computations,
            layout,
            Some(ForeProfile::Line(profile)),
            true,
            nodes,
            cords,
            routes,
            calls,
            &mut startup_order,
        )
    }
}
