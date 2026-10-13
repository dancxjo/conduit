//! Bounded numerical algorithms. Semantic denotations remain `MathValue`.
//!
//! These owned algorithms are preparation-time computations until a separately
//! admitted runtime workspace proves its storage and allocation bounds.
mod binary64;
mod certificate;
mod enclosure;
mod interval;
pub use binary64::{
    Binary64Flags, Binary64Operation, Binary64Result, Binary64Rounding, Binary64Value,
    NumericRealizationWork,
};
pub use certificate::{
    certify_computation, certify_computation_for_algorithm, verify_computation_claim,
    verify_encoded_computation_claim, Binary64StepEvidence, BooleanStepEvidence, I64StepEvidence,
    NumericAlgorithm, NumericCertificateClaim, NumericEngineRefusal, NumericResultEvidence,
    VerifiedNumericWitness,
};
pub use enclosure::enclose_math_value;
pub use interval::{enclose_pi, enclose_sin_cos_radians, enclose_sqrt, RationalInterval};
#[cfg(test)]
mod certificate_tests;
#[cfg(test)]
mod tests;

mod prepared;
pub use prepared::PreparedNumericOutput;

mod artifact;
pub use artifact::{numeric_engine_artifact_id, numeric_engine_identity};
