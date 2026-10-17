use crate::Error;
use burn::{
    grad_clipping::GradientClippingConfig,
    optim::{AdamWConfig, ModuleOptimizer},
};
use serde::{Deserialize, Serialize};

/// v1 supports AdamW, constant learning rate, F32, and per-step seeded randomness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptimizerRecipe {
    pub learning_rate: f64,
    pub weight_decay: f32,
    pub gradient_clip: f32,
    pub seed: u64,
}
impl OptimizerRecipe {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.learning_rate.is_finite()
            || self.learning_rate <= 0.0
            || !(self.learning_rate as f32).is_finite()
            || self.learning_rate as f32 <= 0.0
            || !self.weight_decay.is_finite()
            || self.weight_decay < 0.0
            || !self.gradient_clip.is_finite()
            || self.gradient_clip <= 0.0
        {
            return Err(Error::InvalidDescriptor);
        }
        Ok(())
    }
    pub(crate) fn optimizer(&self) -> Result<ModuleOptimizer, Error> {
        self.validate()?;
        Ok(AdamWConfig::new()
            .with_weight_decay(self.weight_decay)
            .with_grad_clipping(Some(GradientClippingConfig::Norm(self.gradient_clip)))
            .init())
    }
}
