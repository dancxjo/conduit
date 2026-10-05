//! Stable machine-readable refusals for native protocol Host Calls.
use crate::{
    expression_host_call::ExpressionCallRefusal, i2c_base::owner::I2cCallRefusal,
    structured_selector_host_call::SelectorCallRefusal,
};
use conduit_composite::KernelCompositeError;
use conduit_kernel::{Failure, FailureCode};

#[derive(Debug)]
pub enum ProtocolCallRefusal {
    InvalidPlan,
    Unsupported,
    Kernel(KernelCompositeError),
    Expression(ExpressionCallRefusal),
    Selector(SelectorCallRefusal),
    I2c(I2cCallRefusal),
    Clock(crate::monotonic_clock::owner::ClockCallRefusal),
}

impl ProtocolCallRefusal {
    pub fn failure(&self) -> Failure {
        use crate::monotonic_clock::owner::ClockCallRefusal as Clock;
        use ExpressionCallRefusal as Expression;
        use I2cCallRefusal as I2c;
        use SelectorCallRefusal as Selector;
        let (code, detail) = match self {
            Self::InvalidPlan => (FailureCode::InvalidLifecycle, 1020),
            Self::Unsupported => (FailureCode::HostCallDenied, 1021),
            Self::Kernel(_) => (FailureCode::HostCallFailed, 1022),
            Self::Expression(
                Expression::WrongBinding | Expression::StaleRequest | Expression::InvalidProgram,
            ) => (FailureCode::InvalidLifecycle, 1023),
            Self::Expression(Expression::InvalidInput) => (FailureCode::InvalidInput, 1024),
            Self::Expression(Expression::SequenceExhausted) => {
                (FailureCode::IdentityCapacityExhausted, 1025)
            }
            Self::Expression(Expression::Cancelled) => (FailureCode::Cancelled, 1026),
            Self::Expression(Expression::Evaluation(_)) => (FailureCode::HostCallFailed, 1027),
            Self::Selector(
                Selector::WrongBinding | Selector::InvalidSelector | Selector::StaleRequest,
            ) => (FailureCode::InvalidLifecycle, 1040),
            Self::Selector(Selector::InvalidInput) => (FailureCode::InvalidInput, 1041),
            Self::Selector(Selector::SequenceExhausted) => {
                (FailureCode::IdentityCapacityExhausted, 1042)
            }
            Self::Selector(Selector::Cancelled) => (FailureCode::Cancelled, 1043),
            Self::Selector(Selector::Selection(_)) => (FailureCode::InvalidInput, 1044),
            Self::Clock(Clock::WrongBinding | Clock::StaleRequest | Clock::Pending) => {
                (FailureCode::InvalidLifecycle, 1060)
            }
            Self::Clock(Clock::Possession | Clock::Capability(_)) => {
                (FailureCode::HostCallDenied, 1061)
            }
            Self::Clock(Clock::SequenceExhausted) => (FailureCode::IdentityCapacityExhausted, 1062),
            Self::Clock(Clock::Cancelled) => (FailureCode::Cancelled, 1063),
            Self::Clock(Clock::Canonical(_)) => (FailureCode::InvalidInput, 1064),
            Self::I2c(I2c::WrongBinding | I2c::StaleRequest) => {
                (FailureCode::InvalidLifecycle, 1030)
            }
            Self::I2c(I2c::Possession | I2c::Capability(_)) => (FailureCode::HostCallDenied, 1031),
            Self::I2c(I2c::SequenceExhausted) => (FailureCode::IdentityCapacityExhausted, 1032),
            Self::I2c(I2c::Decode(_)) => (FailureCode::InvalidInput, 1033),
            Self::I2c(I2c::Result(_) | I2c::Canonical(_)) => (FailureCode::HostCallFailed, 1034),
        };
        Failure { code, detail }
    }
}
