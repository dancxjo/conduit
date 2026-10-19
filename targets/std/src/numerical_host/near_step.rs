//! One finite input-consuming numerical projection through the existing kernel.
use super::near_encoding::PreparedNearEncoding;
use conduit_core::{projection::*, *};
use conduit_kernel::{
    scheduler::{SchedulerError, StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::{NearQuantityAlgorithm, NUMERICAL_NEAR_MAX_BYTES};
pub const NEAR_NUMERICAL_WORK: u64 = 200_000_000;
// Decoder/composer/report validation have separate existing finite work meters.
pub const NEAR_TOTAL_NUMERICAL_WORK: u64 =
    NEAR_NUMERICAL_WORK + 32 * MATHEMATICAL_VALUE_VALIDATION_WORK;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum NearRefusal {
    Input = 1,
    Physical = 2,
    Tolerance = 3,
    Range = 4,
    Work = 5,
    Encoding = 6,
    Lifecycle = 7,
    Io = 8,
    UnsupportedCoordinate = 9,
    UnsupportedEncoding = 10,
    Extent = 11,
}
impl NearRefusal {
    pub fn name(self) -> &'static str {
        match self {
            Self::Input => "numeric/near-input-refused@1",
            Self::Physical => "numeric/near-physical-limit-mismatch@1",
            Self::Tolerance => "numeric/near-tolerance-exceeded@1",
            Self::Range => "numeric/near-finite-range-refused@1",
            Self::Work => "numeric/near-work-exhausted@1",
            Self::Encoding => "numeric/near-output-envelope-refused@1",
            Self::Lifecycle => "numeric/near-lifecycle-refused@1",
            Self::Io => "numeric/near-io-refused@1",
            Self::UnsupportedCoordinate => "numeric/near-symbolic-limit-coordinate-unsupported@1",
            Self::UnsupportedEncoding => "numeric/near-wire-representation-unsupported@1",
            Self::Extent => "numeric/near-numerical-extent-bound@1",
        }
    }
}
fn numeric(why: ExactNumericRefusal) -> NearRefusal {
    match why {
        ExactNumericRefusal::WorkLimit => NearRefusal::Work,
        ExactNumericRefusal::OutOfRange => NearRefusal::Range,
        ExactNumericRefusal::ExtentExceeded => NearRefusal::Extent,
        _ => NearRefusal::Input,
    }
}
fn rational(why: RationalQuantityRefusal) -> NearRefusal {
    match why {
        RationalQuantityRefusal::Numeric(why) => numeric(why),
        RationalQuantityRefusal::Physical(QuantityConversionRefusal::Inexact) => {
            NearRefusal::UnsupportedCoordinate
        }
        RationalQuantityRefusal::Physical(QuantityConversionRefusal::Overflow) => {
            NearRefusal::Extent
        }
        _ => NearRefusal::Physical,
    }
}
fn quantity(why: QuantityValueRefusal) -> NearRefusal {
    match why {
        QuantityValueRefusal::Rational(why) => rational(why),
        _ => NearRefusal::Input,
    }
}
fn projection(why: QuantityBinary32ProjectionRefusal) -> NearRefusal {
    match why {
        QuantityBinary32ProjectionRefusal::PhysicalToleranceMismatch => NearRefusal::Physical,
        QuantityBinary32ProjectionRefusal::Numeric(Binary32Refusal::Numeric(why)) => numeric(why),
        QuantityBinary32ProjectionRefusal::Numeric(Binary32Refusal::ToleranceExceeded) => {
            NearRefusal::Tolerance
        }
        QuantityBinary32ProjectionRefusal::Numeric(Binary32Refusal::NegativeTolerance) => {
            NearRefusal::Physical
        }
    }
}
fn fail(why: NearRefusal) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: if why == NearRefusal::Work {
            FailureCode::WorkBudgetExhausted
        } else if why == NearRefusal::Lifecycle {
            FailureCode::InvalidLifecycle
        } else if matches!(why, NearRefusal::Io | NearRefusal::Encoding) {
            FailureCode::StorageExhausted
        } else {
            FailureCode::InvalidInput
        },
        detail: why as u16,
    })
}
fn io_failure(why: SchedulerError) -> StepOutcome {
    if why == SchedulerError::StepFuelExceeded {
        fail(NearRefusal::Work)
    } else {
        fail(NearRefusal::Io)
    }
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
// Keep the installed driver envelope small. All state storage is allocated
// and measured during preparation, before the kernel starts Play.
pub struct NearQuantityBack {
    state: Box<NearState>,
    staged: bool,
    complete: bool,
}
struct NearState {
    requested_limit: Quantity,
    limit: RationalQuantity,
    algorithm: NearQuantityAlgorithm,
    encoding: PreparedNearEncoding,
    mathematical: Option<PreparedMathematicalQuantityBinary32>,
}
impl NearQuantityBack {
    pub fn new(
        requested_limit: Quantity,
        algorithm: NearQuantityAlgorithm,
    ) -> Result<Self, String> {
        let limit = RationalQuantity::from_quantity(
            requested_limit,
            &mut ExactNumericWork::new(NEAR_NUMERICAL_WORK),
        )
        .map_err(|why| format!("numeric/near-limit-invalid@1: {why:?}"))?;
        if limit.coordinate().is_negative() || limit.role() == QuantityRole::Point {
            return Err(NearRefusal::Physical.name().into());
        }
        Ok(Self {
            state: Box::new(NearState {
                requested_limit,
                limit,
                algorithm,
                mathematical: if algorithm == NearQuantityAlgorithm::EnclosedBinary32 {
                    Some(
                        PreparedMathematicalQuantityBinary32::new(
                            mathematical::requirements(),
                            Binary32Rounding::NearestTiesEven,
                            limit,
                            NEAR_NUMERICAL_WORK,
                        )
                        .map_err(|why| format!("mathematical near preparation: {why:?}"))?,
                    )
                } else {
                    None
                },
                encoding: if algorithm == NearQuantityAlgorithm::EnclosedBinary32 {
                    PreparedNearEncoding::new_mathematical()
                } else {
                    PreparedNearEncoding::new()
                }
                .map_err(|why| format!("numeric/near-output-preparation@1: {why:?}"))?,
            }),
            staged: false,
            complete: false,
        })
    }
    pub fn retained_capacity_bytes(&self) -> usize {
        core::mem::size_of::<NearState>()
            + self.state.encoding.prepared_heap_bytes()
            + self.state.mathematical.as_ref().map_or(0, |p| {
                p.retained_payload_bytes()
                    - core::mem::size_of::<PreparedMathematicalQuantityBinary32>()
            })
    }
    pub const fn output_bound() -> u32 {
        NUMERICAL_NEAR_MAX_BYTES
    }
    pub fn input_bound(&self) -> usize {
        if self.state.mathematical.is_some() {
            MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES
        } else {
            QUANTITY_VALUE_MAX_ENCODED_BYTES
        }
    }
    // This computes from the actual input, not a startup-frozen receipt.
    #[inline(never)]
    fn evaluate(&mut self, bytes: &[u8]) -> Result<(), NearRefusal> {
        if self.state.mathematical.is_some() && bytes.first() == Some(&3) {
            return self.evaluate_mathematical(bytes);
        }
        let mut work = ExactNumericWork::new(NEAR_NUMERICAL_WORK);
        let view = QuantityValueProfile::Exact
            .decode(QUANTITY_INFO_ID, bytes, &mut work)
            .map_err(|why| match why {
                QuantityValueContractRefusal::Encoding(why) => quantity(why),
                QuantityValueContractRefusal::UnsupportedRepresentation { .. } => {
                    NearRefusal::UnsupportedEncoding
                }
                _ => NearRefusal::Input,
            })?;
        let source = view.rational(&mut work).map_err(quantity)?;
        // Convert only the error-distance coordinates. The source Unit and its
        // original bytes remain unchanged and both requested/effective limits
        // are encoded, preserving this separate exact coordinate decision.
        let limit = self
            .state
            .limit
            .convert_to_unit(source.unit(), &mut work)
            .map_err(rational)?;
        let error_role = if source.role() == QuantityRole::Point {
            QuantityRole::Delta
        } else {
            source.role()
        };
        if limit.role() != error_role || limit.coordinate().is_negative() {
            return Err(NearRefusal::Physical);
        }
        let algorithm = if self.state.algorithm == NearQuantityAlgorithm::EnclosedBinary32 {
            NearQuantityAlgorithm::Binary32
        } else {
            self.state.algorithm
        };
        let projected = match algorithm {
            NearQuantityAlgorithm::EnclosedBinary32 => unreachable!("normalized rational strategy"),
            NearQuantityAlgorithm::ExactRational => None,
            NearQuantityAlgorithm::Binary32 => Some(
                PreparedQuantityBinary32Projection::prepare(
                    source,
                    Binary32Rounding::NearestTiesEven,
                    limit,
                    &mut work,
                )
                .map_err(projection)?,
            ),
        };
        let ids = ProjectionIds::new(
            bytes,
            source,
            self.state.requested_limit,
            projected.as_ref(),
        );
        if let Some(p) = &projected {
            let facts = p.facts();
            let target = p.target();
            let policy = QuantityBinary32ProjectionPolicy {
                maximum_error: limit,
                rounding: Binary32Rounding::NearestTiesEven,
            };
            checked_report(
                &QuantityBinary32ProjectionDomain,
                &policy,
                p,
                &target,
                &facts,
                &ids,
            )?;
        } else {
            let facts = ExactNearDomain.facts();
            checked_report(
                &ExactNearDomain,
                &ExactNearDomain,
                &source,
                &source,
                &facts,
                &ids,
            )?;
        }
        self.state
            .encoding
            .encode(
                source,
                bytes,
                self.state.requested_limit,
                limit,
                algorithm,
                projected.as_ref(),
            )
            .map_err(|_| NearRefusal::Encoding)?;
        Ok(())
    }
}
impl<const PORTS: usize> StepBack<PORTS> for NearQuantityBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if self.staged {
            return fail(NearRefusal::Lifecycle);
        }
        let Some(value) = io.input(PortId(0)) else {
            if !io.input_closed(PortId(0)) {
                return StepOutcome::Await;
            }
            if let Err(why) = io.consume_closed(PortId(0)) {
                return io_failure(why);
            }
            self.complete = true;
            return StepOutcome::Complete;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if value.byte_len as usize > self.input_bound() {
            return fail(NearRefusal::Input);
        }
        let Some(input) = bytes.input(PortId(0)) else {
            return fail(NearRefusal::Input);
        };
        if let Err(why) = io.consume_fuel(1) {
            return io_failure(why);
        }
        if let Err(why) = self.evaluate(input) {
            return fail(why);
        }
        if let Err(why) = io.consume(PortId(0)) {
            return io_failure(why);
        }
        if let Err(why) = io.send_prepared(PortId(0), self.state.encoding.encoded().len() as u32) {
            return io_failure(why);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then_some(self.state.encoding.encoded())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.complete = true;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.complete = true;
        self.staged = false;
    }
}

fn checked_report<'a, D: ProjectionDomain, P: ProjectionPolicy<D>>(
    domain: &'a D,
    policy: &'a P,
    source: &'a D::Source,
    target: &'a D::Target,
    facts: &'a [ProjectionFact<'a, D>],
    ids: &'a ProjectionIds,
) -> Result<(), NearRefusal> {
    // Zero-loss reports also require the actual consuming parameters to pass.
    if !policy.permits(source, target, facts) {
        return Err(NearRefusal::Tolerance);
    }
    let id = |s: &'a [u8]| text(core::str::from_utf8(s).unwrap());
    let report = ProjectionReport::new(
        domain,
        policy,
        ProjectionInput {
            basis: ProjectionBasis {
                report: id(&ids.report),
                source: id(&ids.source),
                target: Some(id(&ids.target)),
                projector: domain.target_contract(),
                requested_route: text("math/near"),
                boundary: None,
            },
            source,
            target: Some(target),
            facts,
            native: &[],
            scores: &[],
            admitted: &[],
            attempts: &[],
            selected_attempt: None,
            mechanism: ProjectionMechanism::Completed,
            diagnostics: &[],
        },
    )
    .map_err(|_| NearRefusal::Encoding)?;
    report
        .require_policy(policy.identity())
        .map_err(|_| NearRefusal::Tolerance)?;
    Ok(())
}
struct ExactNearDomain;
impl ExactNearDomain {
    fn facts(self) -> [ProjectionFact<'static, Self>; 3] {
        [
            ProjectionFact::Preserved {
                obligation: text("mathematical-denotation"),
            },
            ProjectionFact::Transformed {
                obligation: text("numeric-representation"),
                law: text("exact-rational-qualified-coordinate@1"),
            },
            ProjectionFact::Preserved {
                obligation: text("physical-coordinate-contract"),
            },
        ]
    }
}
impl ProjectionDomain for ExactNearDomain {
    type Source = RationalQuantity;
    type Target = RationalQuantity;
    type Detail = ();
    fn source_contract(&self) -> ProjectionText<'_> {
        text(RATIONAL_QUANTITY_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("numeric/near-exact-rational@1")
    }
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        target==Some(source)&&facts.len()==3&&native.is_empty()&&scores.is_empty()&&mechanism==ProjectionMechanism::Completed
            &&facts.iter().any(|f|matches!(f,ProjectionFact::Preserved {obligation} if obligation.as_str()=="mathematical-denotation"))
            &&facts.iter().any(|f|matches!(f,ProjectionFact::Preserved {obligation} if obligation.as_str()=="physical-coordinate-contract"))
            &&facts.iter().any(|f|matches!(f,ProjectionFact::Transformed {obligation,law} if obligation.as_str()=="numeric-representation"&&law.as_str()=="exact-rational-qualified-coordinate@1"))
    }
}
impl ProjectionPolicy<ExactNearDomain> for ExactNearDomain {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/near-exact-rational-policy@1")
    }
    fn permits(
        &self,
        source: &RationalQuantity,
        target: &RationalQuantity,
        facts: &[ProjectionFact<'_, Self>],
    ) -> bool {
        self.validate(
            source,
            Some(target),
            facts,
            &[],
            &[],
            ProjectionMechanism::Completed,
        )
    }
}

#[cfg(test)]
mod tests;

mod mathematical;

#[cfg(test)]
mod mathematical_tests;

mod projection_ids;
use projection_ids::ProjectionIds;

pub(crate) use mathematical::requirements as mathematical_near_requirements;
