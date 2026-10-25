//! Borrowed rich/verified representations for the same Body validation and hash.
use super::verified_inputs::{
    VerifiedBodyMaskChain, VerifiedBodyMaskTopology, VerifiedBodyPlotPlan,
};
use super::*;
#[derive(Clone, Copy)]
pub(super) enum CorePlanRef<'a> {
    Rich(&'a Plan),
    Verified(&'a conduit_core::VerifiedPlanMetadata),
}
impl<'a> CorePlanRef<'a> {
    pub(super) fn metadata(self) -> &'a Plan {
        match self {
            Self::Rich(p) => p,
            Self::Verified(p) => p.preparation_metadata(),
        }
    }
    pub(super) fn valid(self) -> bool {
        match self {
            Self::Rich(p) => verify_plan(p),
            Self::Verified(_) => true,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) struct PlotRef<'a> {
    pub(super) plot: &'a ResidentPlot,
    pub(super) plan: CorePlanRef<'a>,
}
impl<'a> PlotRef<'a> {
    pub(super) fn rich(p: &'a BodyPlotPlan) -> Self {
        Self {
            plot: &p.plot,
            plan: CorePlanRef::Rich(&p.plan),
        }
    }
    pub(super) fn verified(p: &'a VerifiedBodyPlotPlan<'a>) -> Self {
        Self {
            plot: &p.plot,
            plan: CorePlanRef::Verified(p.plan),
        }
    }
}
#[derive(Clone, Copy)]
pub(super) struct ChainRef<'a> {
    pub(super) plan: CorePlanRef<'a>,
    pub(super) stages: &'a [PlacementId],
}
#[derive(Clone, Copy)]
enum Chains<'a> {
    Rich(&'a [BodyMaskChainPlan]),
    Verified(&'a [VerifiedBodyMaskChain<'a>]),
}
#[derive(Clone, Copy)]
pub(super) struct MaskRef<'a> {
    pub(super) face: &'a BodyFaceSelector,
    chains: Chains<'a>,
}
impl<'a> MaskRef<'a> {
    pub(super) fn rich(m: &'a BodyMaskTopology) -> Self {
        Self {
            face: &m.face,
            chains: Chains::Rich(&m.chains),
        }
    }
    pub(super) fn verified(m: &'a VerifiedBodyMaskTopology<'a>) -> Self {
        Self {
            face: &m.face,
            chains: Chains::Verified(&m.chains),
        }
    }
    pub(super) fn chains(self) -> impl ExactSizeIterator<Item = ChainRef<'a>> + Clone {
        let count = match self.chains {
            Chains::Rich(c) => c.len(),
            Chains::Verified(c) => c.len(),
        };
        (0..count).map(move |index| match self.chains {
            Chains::Rich(c) => ChainRef {
                plan: CorePlanRef::Rich(&c[index].plan),
                stages: &c[index].stage_placement_ids,
            },
            Chains::Verified(c) => ChainRef {
                plan: CorePlanRef::Verified(c[index].plan),
                stages: &c[index].stage_placement_ids,
            },
        })
    }
}
