//! Private placement metadata plus immutable checked configuration, never a
//! partial public Plan. Ordinary admission precedes canonical fingerprinting.
use super::*;
use crate::validated_planning::retention::ConfigurationRetention;
use conduit_core::{
    ConfigurationEntry, FragmentCommitment, FragmentFingerprintBuilder, FragmentId, PlacementId,
    PlanCompletionPolicy, PlanFragment, PlanId, PlannedGear, PlotIdentity,
};

/// Preparation representation, not executable material or an authority witness.
/// Configuration remains in the authenticated immutable Source image. Public
/// placement access reconstructs complete records; stripped metadata cannot
/// escape as a purported sealed Plan. No incoming serialized constructor.
pub struct CompactIndexedPlan<'a> {
    image: &'a IndexedSourceImage<'a>,
    identity: PlotIdentity,
    completion: PlanCompletionPolicy,
    fragments: Vec<PlanFragment>,
    indices: BTreeMap<PlacementId, usize>,
    plan_id: PlanId,
}
impl CompactIndexedPlan<'_> {
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    pub fn fragment_count(&self) -> usize {
        self.fragments.len()
    }
    pub fn fragment_id(&self, index: usize) -> Option<&FragmentId> {
        self.fragments.get(index).map(|f| &f.fragment_id)
    }
    pub fn placement_count(&self, fragment: usize) -> Option<usize> {
        self.fragments.get(fragment).map(|f| f.placements.len())
    }
    fn configuration(&self, gear: &PlannedGear) -> Result<Vec<ConfigurationEntry>, PlannerError> {
        source_configuration(self.image, &self.indices, gear)
    }

    fn fragment_fingerprint(&self, fragment: &PlanFragment) -> Result<FragmentId, PlannerError> {
        let mut hash = FragmentFingerprintBuilder::new(fragment.into(), fragment.placements.len());
        for gear in &fragment.placements {
            let configuration = self.configuration(gear)?;
            hash.push_placement_with_configuration(gear, &configuration)
                .map_err(|_| PlannerError::InvalidPlotIdentity("compact placement count".into()))?;
        }
        hash.finish(fragment.into())
            .map_err(|_| PlannerError::InvalidPlotIdentity("incomplete compact fragment".into()))
    }
    /// Materialize one complete placement for a bounded preparation step.
    /// The callback confers no capability possession or execution permission.
    pub fn with_placement<R>(
        &self,
        fragment: usize,
        placement: usize,
        read: impl FnOnce(&PlannedGear) -> R,
    ) -> Result<R, PlannerError> {
        let metadata = self
            .fragments
            .get(fragment)
            .and_then(|f| f.placements.get(placement))
            .ok_or_else(|| PlannerError::InvalidPlotIdentity("compact placement index".into()))?;
        let mut complete = metadata.clone();
        complete.configuration = self.configuration(metadata)?;
        Ok(read(&complete))
    }
    /// Consume compact preparation into Core-verified immutable lowering
    /// material. Rechecks streamed fingerprints and every shared structural
    /// invariant without constructing a complete rich Plan or issuing effects.
    pub fn into_verified_metadata(
        self,
    ) -> Result<conduit_core::VerifiedPlanMetadata, PlannerError> {
        let Self {
            image,
            identity,
            completion,
            fragments,
            indices,
            plan_id,
        } = self;
        let plan = Plan {
            plan_id,
            source_document_id: identity.source_document_id,
            checked_plot_id: identity.checked_plot_id,
            expanded_plot_id: identity.expanded_plot_id,
            completion_policy: completion,
            realization_backs: Vec::new(),
            activations: Vec::new(),
            activation_preparations: Vec::new(),
            fragments,
        };
        conduit_core::verify_plan_metadata_with_configurations(plan, |gear| {
            source_configuration(image, &indices, gear)
        })
        .map_err(|refusal| match refusal {
            conduit_core::PlanMetadataRefusal::Configuration(error) => error,
            _ => PlannerError::InvalidPlotIdentity("compact whole-Plan verification".into()),
        })
    }
    /// Hosted inspection/conformance entrance. Deliberately allocates a complete
    /// rich Plan; a constrained target must not use this for startup.
    pub fn materialize(&self) -> Result<Plan, PlannerError> {
        let mut fragments = self.fragments.clone();
        for fragment in &mut fragments {
            for gear in &mut fragment.placements {
                gear.configuration = self.configuration(gear)?;
            }
        }
        let plan = conduit_core::seal_plan_with_realization_backs_and_completion(
            self.identity.clone(),
            self.completion,
            Vec::new(),
            fragments,
        );
        if plan.plan_id != self.plan_id
            || plan
                .fragments
                .iter()
                .zip(&self.fragments)
                .any(|(a, b)| a.fragment_id != b.fragment_id)
        {
            return Err(PlannerError::InvalidPlotIdentity(
                "compact materialization identity".into(),
            ));
        }
        Ok(plan)
    }
}

/// Same ordinary checks and canonical identities, retaining Source configuration
/// by index instead of retaining every decoded program. The checked Make format's
/// explicit activation/pool/Plot-Back refusals still apply.
#[allow(clippy::too_many_arguments)]
pub fn plan_compact_with_connection_limits<'a>(
    image: &'a IndexedSourceImage<'a>,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<CompactIndexedPlan<'a>, PlannerError> {
    plan_compact_inputs(
        image,
        hosts,
        placements,
        bases,
        options,
        connection_limits,
        None,
    )
}
/// Preserve the same admitted Fore as rich authoring planning while retaining
/// Source configurations by index. This grants no ingress or native authority.
#[allow(clippy::too_many_arguments)]
pub fn plan_compact_authoring_with_connection_limits<'a>(
    image: &'a IndexedSourceImage<'a>,
    boundary: &super::IndexedAuthoringBoundary,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
    boundary_limits: &BTreeMap<crate::ForeBoundaryKey, ConnectionQueueLimits>,
) -> Result<CompactIndexedPlan<'a>, PlannerError> {
    plan_compact_inputs(
        image,
        hosts,
        placements,
        bases,
        options,
        connection_limits,
        Some((boundary, boundary_limits)),
    )
}
#[allow(clippy::too_many_arguments)]
fn plan_compact_inputs<'a>(
    image: &'a IndexedSourceImage<'a>,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
    boundary: Option<(
        &super::IndexedAuthoringBoundary,
        &BTreeMap<crate::ForeBoundaryKey, ConnectionQueueLimits>,
    )>,
) -> Result<CompactIndexedPlan<'a>, PlannerError> {
    let metadata = image.metadata().map_err(|e| {
        PlannerError::InvalidPlotIdentity(alloc::format!("indexed Source metadata: {e:?}"))
    })?;
    let identity = metadata.identity();
    let completion = crate::plan_completion_policy(metadata.completion());
    let mut retention = ConfigurationRetention::new(true);
    let mut fragments = crate::validated_planning::plan_fragments_with_connection_limits(
        PlanningInput {
            source_document_id: &identity.source_document_id,
            checked_plot_id: &identity.checked_plot_id,
            expanded_plot_id: &identity.expanded_plot_id,
            name: metadata.name(),
            completion: metadata.completion(),
            gears: PlanningGears::indexed(image),
            connections: metadata.connections(),
        },
        hosts,
        placements,
        bases,
        options,
        connection_limits,
        &mut retention,
    )?;
    crate::canonical::attach_fragment_source_spans(metadata.provenance(), &mut fragments)?;
    if let Some((boundary, limits)) = boundary {
        boundary.attach(&identity, &mut fragments, limits)?;
    }
    // Retire decoded metadata before the final one-Gear canonical encoding pass.
    drop(metadata);
    let mut compact = CompactIndexedPlan {
        image,
        identity,
        completion,
        fragments,
        indices: retention.indices,
        plan_id: "".into(),
    };
    for index in 0..compact.fragments.len() {
        let id = compact.fragment_fingerprint(&compact.fragments[index])?;
        compact.fragments[index].fragment_id = id;
    }
    let mut commitments = compact
        .fragments
        .iter()
        .map(|f| FragmentCommitment {
            host_id: f.host_id.clone(),
            fragment_id: f.fragment_id.clone(),
        })
        .collect::<Vec<_>>();
    commitments.sort();
    compact.plan_id =
        conduit_core::compute_plan_fingerprint(&compact.identity, &[], &[], &commitments, &[]);
    for fragment in &mut compact.fragments {
        fragment.plan_id = compact.plan_id.clone();
        fragment.plan_fragments = commitments.clone();
    }
    Ok(compact)
}

fn source_configuration(
    image: &IndexedSourceImage<'_>,
    indices: &BTreeMap<PlacementId, usize>,
    gear: &PlannedGear,
) -> Result<Vec<ConfigurationEntry>, PlannerError> {
    let index = indices.get(&gear.placement_id).ok_or_else(|| {
        PlannerError::InvalidPlotIdentity("compact placement has no Source index".into())
    })?;
    let source = image.gear(*index).map_err(|e| {
        PlannerError::InvalidPlotIdentity(alloc::format!("indexed configuration: {e:?}"))
    })?;
    if source.gear_id != gear.gear_id {
        return Err(PlannerError::InvalidPlotIdentity(
            "compact Source Gear mismatch".into(),
        ));
    }
    Ok(source.configuration)
}
