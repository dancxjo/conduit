//! Finite mathematical coordinates projected into a real TensorF64, with custody.
#[cfg(feature = "kernel-step")]
use alloc::boxed::Box;
use alloc::vec::Vec;
use conduit_core::numeric_realization::*;
use conduit_core::projection::{
    ProjectionBasis, ProjectionInput, ProjectionMechanism, ProjectionReport, ProjectionSummary,
    ProjectionText,
};
use conduit_core::*;
use conduit_data::{tensor_content_digest, TensorAxis, TensorBacking, TensorElement, TensorValue};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};

pub const MAXIMUM_NUMERIC_TENSOR_COORDINATES: usize = 8;
pub const MAXIMUM_NUMERIC_TENSOR_ENCODED_BYTES: usize = 4096;
pub use conduit_core::NUMERIC_TENSOR_MATERIALIZATION_RESERVATION_BYTES;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericTensorRefusal {
    Shape,
    UnsupportedRepresentation,
    Coordinate(NumericEngineRefusal),
    ContentMismatch,
    ResourceBudget,
    Projection,
}
impl From<NumericEngineRefusal> for NumericTensorRefusal {
    fn from(value: NumericEngineRefusal) -> Self {
        Self::Coordinate(value)
    }
}

/// A mathematical computation remains coupled to its consumer requirement and
/// verified numerical transformation. Raw TensorF64 is never this exact input.
struct CoordinateCustody {
    graph: NumericComputation,
    requirement: NumericRequirement,
    source: NumericProjectionSource,
    target: NumericProjectionTarget,
}
impl CoordinateCustody {
    fn projection_summary(&self) -> Result<ProjectionSummary, NumericTensorRefusal> {
        let text = |value| ProjectionText::new(value).expect("reviewed projection vocabulary");
        let domain = NumericProjectionDomain::new(self.requirement.semantics);
        let policy = NumericProjectionPolicy;
        let facts = self.target.projection_facts();
        let report = ProjectionReport::new(
            &domain,
            &policy,
            ProjectionInput {
                basis: ProjectionBasis {
                    report: text("numeric/tensor-coordinate-report@1"),
                    source: text("numeric/ordered-mathematical-coordinate@1"),
                    target: Some(text("data/tensor-f64-coordinate@1")),
                    projector: text("numeric/verified-engine@1"),
                    requested_route: text("numeric/consumer-requirement@1"),
                    boundary: None,
                },
                source: &self.source,
                target: Some(&self.target),
                facts: &facts,
                native: &[],
                scores: &[],
                admitted: &[],
                attempts: &[],
                selected_attempt: None,
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
        .map_err(|_| NumericTensorRefusal::Projection)?;
        report
            .require_policy(text("numeric/consumer-requirement-policy@1"))
            .map_err(|_| NumericTensorRefusal::Projection)?;
        Ok(report.summary())
    }
}
pub struct ProjectedTensorF64 {
    tensor: TensorValue,
    coordinates: Vec<CoordinateCustody>,
    source_identity: [u8; 32],
    reservation_bytes: u32,
}
impl ProjectedTensorF64 {
    pub fn prepare(
        dimensions: &[u64],
        axes: &[TensorAxis],
        computations: &[(NumericComputation, NumericRequirement)],
        offer: &NumericRealizationOffer,
    ) -> Result<Self, NumericTensorRefusal> {
        let count = dimensions
            .iter()
            .try_fold(1u64, |count, value| {
                if *value == 0 {
                    None
                } else {
                    count.checked_mul(*value)
                }
            })
            .ok_or(NumericTensorRefusal::Shape)?;
        let count = usize::try_from(count).map_err(|_| NumericTensorRefusal::Shape)?;
        if count > MAXIMUM_NUMERIC_TENSOR_COORDINATES
            || dimensions.is_empty()
            || axes.len() != dimensions.len()
            || computations.len() != count
            || computations.is_empty()
            || computations.len() > MAXIMUM_NUMERIC_TENSOR_COORDINATES
        {
            return Err(NumericTensorRefusal::Shape);
        }
        for axis in axes {
            TensorAxis::new(axis.identity.clone(), axis.role.clone(), axis.unit.clone())
                .map_err(|_| NumericTensorRefusal::Shape)?;
        }
        if offer.algorithm != NumericAlgorithm::OrderedSoftwareBinary64V1 {
            return Err(NumericTensorRefusal::UnsupportedRepresentation);
        }
        let requirement = &computations[0].1;
        if computations.iter().any(|(_, other)| other != requirement) {
            return Err(NumericTensorRefusal::UnsupportedRepresentation);
        }
        let graphs = computations
            .iter()
            .map(|(graph, _)| graph.clone())
            .collect::<Vec<_>>();
        let witnesses = offer.certify_tensor(&graphs, requirement)?;
        let mut coordinates = Vec::with_capacity(computations.len());
        let mut payload = Vec::with_capacity(computations.len() * 8);
        let mut reservation = NUMERIC_TENSOR_MATERIALIZATION_RESERVATION_BYTES;
        let mut identities = Vec::with_capacity(computations.len() * 64);
        for ((graph, requirement), witness) in computations.iter().zip(witnesses) {
            if requirement.semantics != NumericSemantics::Mathematical
                || (graph.nodes()[graph.root() as usize].semantic_kind != MATH_VALUE_INFO_ID
                    && PortableNumericProfile::from_info_id(
                        &graph.nodes()[graph.root() as usize].semantic_kind,
                    )
                    .is_none())
            {
                return Err(NumericTensorRefusal::UnsupportedRepresentation);
            }
            let NumericResultEvidence::Enclosure(observed) = &witness.claim().result else {
                return Err(NumericTensorRefusal::UnsupportedRepresentation);
            };
            if observed.lower() != observed.upper() {
                return Err(NumericTensorRefusal::UnsupportedRepresentation);
            }
            let step = witness
                .claim()
                .steps
                .last()
                .ok_or(NumericTensorRefusal::UnsupportedRepresentation)?;
            let bits = Binary64Value::from_bits(step.output);
            if bits.exact().map_err(NumericEngineRefusal::from)?.as_ref() != Some(observed.lower())
            {
                return Err(NumericTensorRefusal::ContentMismatch);
            }
            let required = witness.required_memory_reservation_bytes(graph)?;
            reservation = reservation
                .checked_add(required)
                .ok_or(NumericTensorRefusal::ResourceBudget)?;
            identities.extend_from_slice(witness.graph_identity());
            identities.extend_from_slice(witness.requirement_identity());
            payload.extend_from_slice(&step.output.to_le_bytes());
            let source = NumericProjectionSource::new(graph, requirement)?;
            let target = NumericProjectionTarget::new(&source, witness)?;
            let coordinate = CoordinateCustody {
                graph: graph.clone(),
                requirement: requirement.clone(),
                source,
                target,
            };
            coordinate.projection_summary()?;
            coordinates.push(coordinate);
        }
        let tensor = TensorValue {
            element: TensorElement::F64,
            dimensions: BoundedSequence::try_from_iter(dimensions.iter().copied())
                .map_err(|_| NumericTensorRefusal::Shape)?,
            axes: BoundedSequence::try_from_iter(axes.iter().cloned())
                .map_err(|_| NumericTensorRefusal::Shape)?,
            content_digest: tensor_content_digest(&payload),
            backing: TensorBacking::Inline(
                BoundedBytes::new(&payload).ok_or(NumericTensorRefusal::Shape)?,
            ),
        };
        let encoded = tensor.encode().map_err(|_| NumericTensorRefusal::Shape)?;
        if encoded.len() > MAXIMUM_NUMERIC_TENSOR_ENCODED_BYTES {
            return Err(NumericTensorRefusal::Shape);
        }
        // Canonical inline tensor suffix is contentDigest32 + backingTag1 +
        // payloadLength4 + payload. Layout custody excludes realized numbers.
        let layout_end = encoded
            .len()
            .checked_sub(37 + payload.len())
            .ok_or(NumericTensorRefusal::Shape)?;
        let layout = semantic_digest("numeric/tensor-layout@1", &encoded[..layout_end]);
        identities.extend_from_slice(&layout);
        let source_identity = semantic_digest("numeric/tensor-coordinate-custody@1", &identities);
        Ok(Self {
            tensor,
            coordinates,
            source_identity,
            reservation_bytes: reservation,
        })
    }
    pub fn tensor(&self) -> &TensorValue {
        &self.tensor
    }
    pub fn consume_summary(&self) -> Result<conduit_data::TensorSummary, NumericTensorRefusal> {
        self.readmit(&self.tensor)?;
        self.tensor
            .summary()
            .map_err(|_| NumericTensorRefusal::ContentMismatch)
    }
    pub fn source_identity(&self) -> [u8; 32] {
        self.source_identity
    }
    pub fn required_memory_reservation_bytes(&self) -> u32 {
        self.reservation_bytes
    }
    pub fn coordinate_projection_summaries(
        &self,
    ) -> Result<Vec<ProjectionSummary>, NumericTensorRefusal> {
        self.coordinates
            .iter()
            .map(CoordinateCustody::projection_summary)
            .collect()
    }
    pub fn coordinate_projection_evidence(
        &self,
    ) -> impl ExactSizeIterator<Item = (&NumericProjectionSource, &NumericProjectionTarget)> {
        self.coordinates
            .iter()
            .map(|coordinate| (&coordinate.source, &coordinate.target))
    }
    pub fn coordinate_witnesses(&self) -> impl ExactSizeIterator<Item = &VerifiedNumericWitness> {
        self.coordinates
            .iter()
            .map(|coordinate| coordinate.target.witness())
    }
    /// A Tensor consumer readmits the exact materialized bytes and every ordered
    /// numerical certificate; neither matching shape nor a claimed digest suffices.
    pub fn readmit(&self, tensor: &TensorValue) -> Result<(), NumericTensorRefusal> {
        tensor
            .validate()
            .map_err(|_| NumericTensorRefusal::ContentMismatch)?;
        if tensor != &self.tensor {
            return Err(NumericTensorRefusal::ContentMismatch);
        }
        for coordinate in &self.coordinates {
            verify_computation_claim(
                coordinate.target.witness().claim(),
                &coordinate.graph,
                &coordinate.requirement,
            )?;
            coordinate.projection_summary()?;
        }
        Ok(())
    }
}

#[cfg(feature = "kernel-step")]
pub struct NumericTensorBack {
    projection: Box<ProjectedTensorF64>,
    encoded: Vec<u8>,
    consumer_summary: conduit_data::TensorSummary,
    emitted: bool,
}
#[cfg(feature = "kernel-step")]
impl NumericTensorBack {
    pub fn prepare(
        projection: ProjectedTensorF64,
        reserved_bytes: u32,
    ) -> Result<Self, NumericTensorRefusal> {
        if reserved_bytes < projection.required_memory_reservation_bytes() {
            return Err(NumericTensorRefusal::ResourceBudget);
        }
        let consumer_summary = projection.consume_summary()?;
        let encoded = projection
            .tensor()
            .encode()
            .map_err(|_| NumericTensorRefusal::ContentMismatch)?;
        Ok(Self {
            projection: Box::new(projection),
            encoded,
            consumer_summary,
            emitted: false,
        })
    }
    pub fn consumer_summary(&self) -> &conduit_data::TensorSummary {
        &self.consumer_summary
    }
    pub fn projection(&self) -> &ProjectedTensorF64 {
        &self.projection
    }
}
#[cfg(feature = "kernel-step")]
impl<const PORTS: usize> conduit_kernel::scheduler::StepBack<PORTS> for NumericTensorBack {
    fn step(
        &mut self,
        io: &mut conduit_kernel::scheduler::StepIo<PORTS>,
        _: &conduit_kernel::scheduler::StepInputBytes<'_, PORTS>,
    ) -> conduit_kernel::scheduler::StepOutcome {
        use conduit_kernel::{scheduler::StepOutcome, PortId};
        if self.emitted {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io
            .send_prepared(PortId(0), self.encoded.len() as u32)
            .is_err()
        {
            return StepOutcome::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidInput,
                detail: 5392,
            });
        }
        self.emitted = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: conduit_kernel::PortId) -> Option<&[u8]> {
        (port == conduit_kernel::PortId(0)).then_some(self.encoded.as_slice())
    }
    fn cancel(&mut self) {
        self.emitted = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};
    use conduit_core::projection::{ProjectionDisposition, ProjectionFidelity, ProjectionLoss};
    fn input(accuracy: NumericAccuracy) -> (NumericComputation, NumericRequirement) {
        let graph = NumericComputation::new(
            vec![NumericComputationNode::literal(
                MATH_VALUE_INFO_ID.to_string(),
                None,
                &MathValue::rational("1", "10").unwrap(),
            )
            .unwrap()],
            0,
        )
        .unwrap();
        let requirement = NumericRequirement {
            semantics: NumericSemantics::Mathematical,
            representation: NumericRepresentation::Binary64,
            rounding: Binary64Rounding::NearestEven,
            accuracy,
            parameters: Vec::new(),
            budget: NumericExecutionBudget {
                storage_bytes: 1_048_576,
                work_steps: 100_000,
                limbs: 128,
                symbolic_nodes: 128,
            },
            reproducible: true,
            exceptions: NumericExceptionPolicy {
                inexact: true,
                ..Default::default()
            },
        };
        (graph, requirement)
    }
    #[test]
    fn decimal_tensor_requires_certified_loss_custody_and_rejects_changed_content() {
        let strict = input(NumericAccuracy::Exact);
        let offer = NumericRealizationOffer {
            algorithm: NumericAlgorithm::OrderedSoftwareBinary64V1,
            budget: strict.1.budget,
        };
        let axis = TensorAxis {
            role: conduit_data::TensorAxisRole::Feature,
            identity: None,
            unit: None,
        };
        assert!(ProjectedTensorF64::prepare(&[1], &[axis.clone()], &[strict], &offer).is_err());
        let tolerant = input(NumericAccuracy::Absolute(
            MathValue::rational("1", "10000000000000000").unwrap(),
        ));
        let projection = ProjectedTensorF64::prepare(&[1], &[axis], &[tolerant], &offer).unwrap();
        projection.consume_summary().unwrap();
        let summaries = projection.coordinate_projection_summaries().unwrap();
        assert_eq!(
            summaries[0].disposition,
            ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
        );
        assert_eq!(summaries[0].losses[ProjectionLoss::Precision as usize], 1);
        let mut forged = projection.tensor().clone();
        forged.content_digest = tensor_content_digest(&0u64.to_le_bytes());
        forged.backing = TensorBacking::Inline(BoundedBytes::new(&0u64.to_le_bytes()).unwrap());
        assert_eq!(
            projection.readmit(&forged),
            Err(NumericTensorRefusal::ContentMismatch)
        );
        #[cfg(feature = "kernel-step")]
        {
            let reservation = projection.required_memory_reservation_bytes();
            assert!(matches!(
                NumericTensorBack::prepare(projection, reservation - 1),
                Err(NumericTensorRefusal::ResourceBudget)
            ));
        }
    }
}

mod admission;
pub use admission::*;
