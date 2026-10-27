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
pub(super) enum PlotPlanRef<'a> {
    Live(CorePlanRef<'a>),
    Prepared(&'a seal::PreparedVerifiedBodyPlotPlan),
}
impl PlotPlanRef<'_> {
    pub(super) fn plan_id(&self) -> &PlanId {
        match self {
            Self::Live(p) => &p.metadata().plan_id,
            Self::Prepared(p) => p.seal().plan_id(),
        }
    }
    pub(super) fn valid(self) -> bool {
        match self {
            Self::Live(p) => p.valid(),
            Self::Prepared(_) => true,
        }
    }
    pub(super) fn identity_matches(self, plot: &ResidentPlot) -> bool {
        match self {
            Self::Live(p) => {
                let p = p.metadata();
                p.source_document_id == plot.source_document_id
                    && p.checked_plot_id == plot.checked_plot_id
            }
            Self::Prepared(p) => p.plot() == plot,
        }
    }
    pub(super) fn has_placement(self, id: &PlacementId) -> bool {
        match self {
            Self::Live(p) => p
                .metadata()
                .fragments
                .iter()
                .flat_map(|f| &f.placements)
                .any(|p| &p.placement_id == id),
            Self::Prepared(p) => p.has_placement(id),
        }
    }
    pub(super) fn scope(self) -> (usize, bool) {
        match self {
            Self::Live(p) => {
                let p = p.metadata();
                (
                    p.fragments.len(),
                    !p.activations.is_empty() || !p.activation_preparations.is_empty(),
                )
            }
            Self::Prepared(p) => (p.seal().fragments().len(), false),
        }
    }
    pub(super) fn capture(self) -> seal::BodyPlotPlanSeal {
        match self {
            Self::Live(p) => seal::capture_core(p.metadata()),
            Self::Prepared(p) => p.seal().clone(),
        }
    }
}
#[derive(Clone, Copy)]
pub(super) struct PlotRef<'a> {
    pub(super) plot: &'a ResidentPlot,
    pub(super) plan: PlotPlanRef<'a>,
}
impl<'a> PlotRef<'a> {
    pub(super) fn rich(p: &'a BodyPlotPlan) -> Self {
        Self {
            plot: &p.plot,
            plan: PlotPlanRef::Live(CorePlanRef::Rich(&p.plan)),
        }
    }
    pub(super) fn verified(p: &'a VerifiedBodyPlotPlan<'a>) -> Self {
        Self {
            plot: &p.plot,
            plan: PlotPlanRef::Live(CorePlanRef::Verified(p.plan)),
        }
    }
    pub(super) fn prepared(p: &'a seal::PreparedVerifiedBodyPlotPlan) -> Self {
        Self {
            plot: p.plot(),
            plan: PlotPlanRef::Prepared(p),
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
