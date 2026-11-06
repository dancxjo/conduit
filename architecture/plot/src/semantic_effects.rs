//! Read-only semantic work inference over the existing checked expansion.
//!
//! Nested Plots have already expanded to exact leaf Gears. Retain their Source
//! paths instead of minting a second graph or an author-trusted effect label.
//! These preparation facts are not realization eligibility or confinement.
use crate::prelude::*;
use crate::{CheckedGear, ExpandedCanonicalPlot, ExpandedGearProvenance, Span};
use conduit_core::{
    ExternalEffectBehavior, PureExpressionFacts, PureExpressionRefusal, ReplayBehavior,
    SemanticDependence, SuspensionBehavior, TemporalStateBehavior, VariabilityBehavior,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectInferenceRefusal {
    Capacity,
    ChangedCheckedExpansion,
    InvalidExpansion(crate::CanonicalExpansionDiagnostic),
    InvalidProvenance(String),
    UnresolvedActivation {
        identity: String,
        span: Span,
    },
    UnresolvedPool(String),
    SemanticFact {
        gear: String,
        span: Span,
        refusal: PureExpressionRefusal,
    },
}

/// Distinct observed laws, with no ordering or substitution between contracts.
/// In particular, transactional/idempotent replay keys cannot become `Exact`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompositionEffectFacts {
    pub external_effects: Vec<ExternalEffectBehavior>,
    pub temporal_state: Vec<TemporalStateBehavior>,
    pub time_dependence: Vec<SemanticDependence>,
    pub random_dependence: Vec<SemanticDependence>,
    pub resource_dependence: Vec<SemanticDependence>,
    pub suspension: Vec<SuspensionBehavior>,
    pub variability: Vec<VariabilityBehavior>,
    pub replay: Vec<ReplayBehavior>,
}

#[derive(Debug)]
pub struct GearEffectFacts<'a> {
    gear: &'a CheckedGear,
    origin: &'a ExpandedGearProvenance,
    facts: Result<PureExpressionFacts, PureExpressionRefusal>,
}
impl GearEffectFacts<'_> {
    pub fn gear(&self) -> &CheckedGear {
        self.gear
    }
    pub fn origin(&self) -> &ExpandedGearProvenance {
        self.origin
    }
    pub fn facts(&self) -> Result<&PureExpressionFacts, EffectInferenceRefusal> {
        self.facts
            .as_ref()
            .map_err(|refusal| self.diagnostic(*refusal))
    }
    fn diagnostic(&self, refusal: PureExpressionRefusal) -> EffectInferenceRefusal {
        EffectInferenceRefusal::SemanticFact {
            gear: self.gear.gear_id.as_str().into(),
            span: self.origin.source_span,
            refusal,
        }
    }
}

/// Borrows the exact validated expansion, preventing mutation while inspected.
/// Unknown contracts remain inspectable leaf refusals; a composition-wide
/// conclusion requires complete coverage, including deferred execution bodies.
#[derive(Debug)]
pub struct InferredPlotEffects<'a> {
    plot: &'a ExpandedCanonicalPlot,
    leaves: Vec<GearEffectFacts<'a>>,
}
impl ExpandedCanonicalPlot {
    /// Bound retained inference by the total graph/provenance item count.
    /// Compiler preparation retains its separate work/storage rules; this
    /// does not alter any existing compiler, Plan or Play admission limit.
    pub fn infer_semantic_effects(
        &self,
        document: &crate::CheckedSyntaxDocument,
        catalog: &crate::ProfileCatalog,
        maximum_items: usize,
    ) -> Result<InferredPlotEffects<'_>, EffectInferenceRefusal> {
        let count = [
            self.gears.len(),
            self.connections.len(),
            self.provenance.len(),
            self.connection_provenance.len(),
            self.activations.len(),
            self.shared_pools.len(),
            self.realization_backs.len(),
        ]
        .into_iter()
        .try_fold(0usize, |total, next| total.checked_add(next));
        if maximum_items == 0 || count.is_none_or(|count| count > maximum_items) {
            return Err(EffectInferenceRefusal::Capacity);
        }
        self.validate_expansion()
            .map_err(EffectInferenceRefusal::InvalidExpansion)?;
        // Graph identity alone does not authenticate attached Kind law metadata.
        // Reuse the actual checker-owned expansion and compare every field,
        // including contracts and provenance, before reading those laws.
        let expected = crate::expand_canonical_plot_for_authoring(document, &self.name, catalog)
            .map_err(EffectInferenceRefusal::InvalidExpansion)?;
        if expected.expanded != *self {
            return Err(EffectInferenceRefusal::ChangedCheckedExpansion);
        }
        let mut leaves = Vec::with_capacity(self.gears.len());
        for gear in &self.gears {
            let mut origins = self
                .provenance
                .iter()
                .filter(|row| row.gear_id == gear.gear_id.as_str());
            let origin = origins.next().ok_or_else(|| {
                EffectInferenceRefusal::InvalidProvenance(gear.gear_id.as_str().into())
            })?;
            if origins.next().is_some() {
                return Err(EffectInferenceRefusal::InvalidProvenance(
                    gear.gear_id.as_str().into(),
                ));
            }
            leaves.push(GearEffectFacts {
                gear,
                origin,
                facts: gear.semantic_contract.work_facts(),
            });
        }
        if self.provenance.len() != leaves.len() {
            return Err(EffectInferenceRefusal::InvalidProvenance(
                "unmapped Source origin".into(),
            ));
        }
        Ok(InferredPlotEffects { plot: self, leaves })
    }
}
impl InferredPlotEffects<'_> {
    pub fn plot(&self) -> &ExpandedCanonicalPlot {
        self.plot
    }
    pub fn leaves(&self) -> &[GearEffectFacts<'_>] {
        &self.leaves
    }
    fn require_complete_bodies(&self) -> Result<(), EffectInferenceRefusal> {
        if let Some(activation) = self.plot.activations.first() {
            return Err(EffectInferenceRefusal::UnresolvedActivation {
                identity: activation.activation_id.clone(),
                span: activation.source_span,
            });
        }
        if let Some(pool) = self.plot.shared_pools.first() {
            return Err(EffectInferenceRefusal::UnresolvedPool(
                pool.pool_id.as_str().into(),
            ));
        }
        Ok(())
    }
    pub fn summary(&self) -> Result<CompositionEffectFacts, EffectInferenceRefusal> {
        self.require_complete_bodies()?;
        let mut result = CompositionEffectFacts::default();
        for leaf in &self.leaves {
            let facts = leaf.facts()?;
            distinct(&mut result.external_effects, facts.external_effects);
            distinct(&mut result.temporal_state, facts.temporal_state);
            distinct(&mut result.time_dependence, facts.time_dependence);
            distinct(&mut result.random_dependence, facts.random_dependence);
            distinct(&mut result.resource_dependence, facts.resource_dependence);
            distinct(&mut result.suspension, facts.suspension);
            distinct(&mut result.variability, facts.variability);
            distinct(&mut result.replay, facts.replay.clone());
        }
        Ok(result)
    }
    /// Every reachable leaf must satisfy the existing Core semantic rule.
    /// Back/Host eligibility and actual effect authority are separate checks.
    pub fn require_pure_expression(&self) -> Result<(), EffectInferenceRefusal> {
        self.require_complete_bodies()?;
        for leaf in &self.leaves {
            leaf.facts()?
                .require_pure_expression()
                .map_err(|r| leaf.diagnostic(r))?;
        }
        Ok(())
    }
}
fn distinct<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}
