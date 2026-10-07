//! Exact finite categorical linear inference. This owner has no domain policy.
use crate::{
    model_content_digest, ModelArtifact, ModelDimensionConstraint, ModelSignature,
    ModelValueConstraint,
};
use alloc::vec::Vec;
use conduit_data::{
    tensor_content_digest, TensorAxis, TensorAxisRole, TensorBacking, TensorElement, TensorValue,
};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};

pub const CATEGORICAL_I16_ARCHITECTURE: &str = "ai/categorical-linear@1";
pub const CATEGORICAL_I16_FORMAT: &str = "model/categorical-i16-sum@1";
pub const CATEGORICAL_I16_PRECISION: &str = "number/i16-weights-i64-sums@1";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoricalRefusal {
    Artifact,
    Signature,
    Format,
    Bounds,
    Input,
    Index,
}
pub struct IntegerCategoricalModel {
    identity: [u8; 32],
    categories: usize,
    outputs: usize,
    lookups: usize,
    weights: Vec<i16>,
}
impl IntegerCategoricalModel {
    pub fn prepare(
        artifact: &ModelArtifact,
        signature: &ModelSignature,
        bytes: &[u8],
    ) -> Result<Self, CategoricalRefusal> {
        artifact
            .validate(signature)
            .map_err(|_| CategoricalRefusal::Artifact)?;
        if artifact.content_identity() != model_content_digest(bytes)
            || artifact.content.extent.bytes != bytes.len() as u64
        {
            return Err(CategoricalRefusal::Artifact);
        }
        if artifact.architecture_profile != CATEGORICAL_I16_ARCHITECTURE
            || artifact.format_profile != CATEGORICAL_I16_FORMAT
            || artifact.precision_profile != CATEGORICAL_I16_PRECISION
            || bytes.get(..8) != Some(b"CI16SUM1")
        {
            return Err(CategoricalRefusal::Format);
        }
        let scalar = |offset| {
            bytes
                .get(offset..offset + 4)
                .and_then(|v| v.try_into().ok())
                .map(u32::from_le_bytes)
                .map(|v| v as usize)
                .ok_or(CategoricalRefusal::Format)
        };
        let categories = scalar(8)?;
        let outputs = scalar(12)?;
        let lookups = scalar(16)?;
        if !(1..=4096).contains(&categories)
            || !(1..=128).contains(&outputs)
            || !(1..=64).contains(&lookups)
            || categories * outputs * 2 > 65536
            || bytes.len() != 20 + categories * outputs * 2
        {
            return Err(CategoricalRefusal::Bounds);
        }
        if signature.inputs().get().len() != 1 || signature.outputs().get().len() != 1 {
            return Err(CategoricalRefusal::Signature);
        }
        let matches = |value: &ModelValueConstraint, element, count| {
            let ModelValueConstraint::Tensor(v) = value else {
                return false;
            };
            let c = v.constraint();
            c.elements().get().as_slice() == [element]
                && c.axes().get().len() == 1
                && c.axes().get()[0].role == TensorAxisRole::Feature
                && matches!(&c.axes().get()[0].dimension, ModelDimensionConstraint::Fixed(v) if *v.value()==count)
                && *c.maximum_bytes() == count * 8
        };
        if !matches(
            signature.inputs().get()[0].value(),
            TensorElement::U64,
            lookups as u64,
        ) || !matches(
            signature.outputs().get()[0].value(),
            TensorElement::I64,
            outputs as u64,
        ) {
            return Err(CategoricalRefusal::Signature);
        }
        Ok(Self {
            identity: artifact.content_identity(),
            categories,
            outputs,
            lookups,
            weights: bytes[20..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|v| i16::from_le_bytes(*v))
                .collect(),
        })
    }
    /// Preparation-time inspection of the exact learned parameter tensor.
    pub fn weights_tensor(&self) -> Result<TensorValue, CategoricalRefusal> {
        let bytes = self
            .weights
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>();
        Ok(TensorValue {
            element: TensorElement::I16,
            dimensions: BoundedSequence::try_from_iter([
                self.outputs as u64,
                self.categories as u64,
            ])
            .map_err(|_| CategoricalRefusal::Bounds)?,
            axes: BoundedSequence::try_from_iter([
                TensorAxis {
                    role: TensorAxisRole::Feature,
                    identity: Some("output".into()),
                    unit: None,
                },
                TensorAxis {
                    role: TensorAxisRole::Feature,
                    identity: Some("category".into()),
                    unit: None,
                },
            ])
            .map_err(|_| CategoricalRefusal::Bounds)?,
            content_digest: tensor_content_digest(&bytes),
            backing: TensorBacking::Inline(
                BoundedBytes::new(&bytes).ok_or(CategoricalRefusal::Bounds)?,
            ),
        })
    }
    pub fn dimensions(&self) -> (usize, usize, usize) {
        (self.categories, self.outputs, self.lookups)
    }
    /// Conservative absolute output bound for admission into narrower consumers.
    /// Includes i16::MIN without signed absolute-value overflow.
    pub fn maximum_score_magnitude(&self) -> u64 {
        self.weights
            .iter()
            .map(|v| i64::from(*v).unsigned_abs())
            .max()
            .unwrap_or(0)
            * self.lookups as u64
    }
    /// Pure bounded inference into caller-prepared storage. Refusals leave the
    /// output unchanged; the checked limits keep every exact sum within I64.
    pub fn infer_indices_into(
        &self,
        indices: &[u64],
        scores: &mut [i64],
    ) -> Result<(), CategoricalRefusal> {
        if indices.len() != self.lookups || scores.len() != self.outputs {
            return Err(CategoricalRefusal::Input);
        }
        if indices.iter().any(|i| *i >= self.categories as u64) {
            return Err(CategoricalRefusal::Index);
        }
        for (output, score) in scores.iter_mut().enumerate() {
            *score = indices
                .iter()
                .map(|i| i64::from(self.weights[output * self.categories + *i as usize]))
                .sum();
        }
        Ok(())
    }
    pub fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub fn work_units(&self) -> u64 {
        (self.outputs * self.lookups) as u64
    }
    pub fn working_bytes(&self) -> u64 {
        (core::mem::size_of::<TensorValue>() * 2 + (self.lookups + self.outputs) * 8) as u64
    }
    pub fn infer(&self, input: &TensorValue) -> Result<TensorValue, CategoricalRefusal> {
        input.validate().map_err(|_| CategoricalRefusal::Input)?;
        if input.element != TensorElement::U64
            || input.dimensions.as_slice() != [self.lookups as u64]
            || input.axes.as_slice().len() != 1
            || input.axes.as_slice()[0].role != TensorAxisRole::Feature
        {
            return Err(CategoricalRefusal::Input);
        }
        let TensorBacking::Inline(bytes) = &input.backing else {
            return Err(CategoricalRefusal::Input);
        };
        let indices = bytes
            .as_slice()
            .as_chunks::<8>()
            .0
            .iter()
            .map(|v| u64::from_le_bytes(*v))
            .collect::<Vec<_>>();
        if indices.iter().any(|i| *i >= self.categories as u64) {
            return Err(CategoricalRefusal::Index);
        }
        let mut bytes = Vec::with_capacity(self.outputs * 8);
        for output in 0..self.outputs {
            let score = indices
                .iter()
                .map(|i| i64::from(self.weights[output * self.categories + *i as usize]))
                .sum::<i64>();
            bytes.extend_from_slice(&score.to_le_bytes());
        }
        Ok(TensorValue {
            element: TensorElement::I64,
            dimensions: BoundedSequence::try_from_iter([self.outputs as u64])
                .map_err(|_| CategoricalRefusal::Bounds)?,
            axes: BoundedSequence::try_from_iter([TensorAxis {
                role: TensorAxisRole::Feature,
                identity: None,
                unit: None,
            }])
            .map_err(|_| CategoricalRefusal::Bounds)?,
            content_digest: tensor_content_digest(&bytes),
            backing: TensorBacking::Inline(
                BoundedBytes::new(&bytes).ok_or(CategoricalRefusal::Bounds)?,
            ),
        })
    }
}
