//! Host-neutral exact finite monetary semantics without floats or provider symbols.

use conduit_core::{StructuredInfoRefusal, StructuredInfoType};
use core::cmp::Ordering;

pub const FINANCE_FIXED_DECIMAL_TYPE: &str = "FinanceFixedDecimal";
pub const FINANCE_CURRENCY_TYPE: &str = "FinanceCurrency";
pub const FINANCE_MONEY_TYPE: &str = "FinanceMoney";
pub const FINANCE_INSTRUMENT_TYPE: &str = "FinanceCurrencyPair";
pub const FINANCE_INSTANT_TYPE: &str = "FinanceObservedInstant";
pub const FINANCE_FRESHNESS_TYPE: &str = "FinanceQuoteFreshness";
pub const FINANCE_QUOTE_TYPE: &str = "FinanceQuote";
pub const FINANCE_RATE_TYPE: &str = "FinanceRateObservation";
pub const FINANCE_TRANSACTION_EVENT_TYPE: &str = "FinanceTransactionEvent";
pub const FINANCE_TRANSACTION_EVENTS_TYPE: &str = "FinanceTransactionEventsThree";
pub const FINANCE_MONEY_COMPARISON_TYPE: &str = "FinanceMoneyComparison";
pub const FINANCE_MAXIMUM_DECIMAL_SCALE: u8 = 9;
pub const FINANCE_TRANSACTION_EVENT_COUNT: u16 = 3;

pub type FixedDecimal = crate::FinanceFixedDecimal;

pub type Currency = crate::FinanceCurrency;

pub type Money = crate::FinanceMoney;

pub type RateObservation = crate::FinanceRateObservation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinanceRefusal {
    CurrencyMismatch { left: Currency, right: Currency },
    RatePairMismatch,
    Overflow,
    MalformedInfo,
    Structured(StructuredInfoRefusal),
    NativeBinding(conduit_form::rust_binding::NativeBindingRefusal),
}

impl From<StructuredInfoRefusal> for FinanceRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

impl From<conduit_form::rust_binding::NativeBindingRefusal> for FinanceRefusal {
    fn from(value: conduit_form::rust_binding::NativeBindingRefusal) -> Self {
        Self::NativeBinding(value)
    }
}

impl FixedDecimal {
    pub fn checked_add(&self, other: &Self) -> Result<Self, FinanceRefusal> {
        let scale = (*self.scale()).max(*other.scale());
        let left = self.at_scale(scale)?;
        let right = other.at_scale(scale)?;
        Ok(Self::new(
            left.checked_add(right).ok_or(FinanceRefusal::Overflow)?,
            scale,
        )?)
    }

    pub fn checked_cmp(&self, other: &Self) -> Result<Ordering, FinanceRefusal> {
        let scale = (*self.scale()).max(*other.scale());
        Ok(self.at_scale(scale)?.cmp(&other.at_scale(scale)?))
    }

    pub fn checked_mul(&self, other: &Self) -> Result<Self, FinanceRefusal> {
        let scale = self
            .scale()
            .checked_add(*other.scale())
            .ok_or(FinanceRefusal::Overflow)?;
        Ok(Self::new(
            self.coefficient()
                .checked_mul(*other.coefficient())
                .ok_or(FinanceRefusal::Overflow)?,
            scale,
        )?)
    }

    fn at_scale(&self, scale: u8) -> Result<i64, FinanceRefusal> {
        let factor = 10_i64
            .checked_pow(u32::from(scale - *self.scale()))
            .ok_or(FinanceRefusal::Overflow)?;
        self.coefficient()
            .checked_mul(factor)
            .ok_or(FinanceRefusal::Overflow)
    }
}

impl Currency {
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Eur => "eur",
            Self::Gbp => "gbp",
            Self::Usd => "usd",
        }
    }

    pub fn from_tag(tag: &str) -> Result<Self, FinanceRefusal> {
        match tag {
            "eur" => Ok(Self::Eur),
            "gbp" => Ok(Self::Gbp),
            "usd" => Ok(Self::Usd),
            _ => Err(FinanceRefusal::MalformedInfo),
        }
    }
}

pub fn add_money(left: Money, right: Money) -> Result<Money, FinanceRefusal> {
    require_same_currency(left.currency(), right.currency())?;
    Ok(Money::new(
        left.amount().checked_add(right.amount())?,
        *left.currency(),
    )?)
}

pub fn compare_money(left: Money, right: Money) -> Result<Ordering, FinanceRefusal> {
    require_same_currency(left.currency(), right.currency())?;
    left.amount().checked_cmp(right.amount())
}

pub fn convert_money(money: Money, rate: &RateObservation) -> Result<Money, FinanceRefusal> {
    if rate.base() == rate.quote() || money.currency() != rate.base() {
        return Err(FinanceRefusal::RatePairMismatch);
    }
    Ok(Money::new(
        money.amount().checked_mul(rate.rate())?,
        *rate.quote(),
    )?)
}

fn require_same_currency(left: &Currency, right: &Currency) -> Result<(), FinanceRefusal> {
    if left != right {
        return Err(FinanceRefusal::CurrencyMismatch {
            left: *left,
            right: *right,
        });
    }
    Ok(())
}

pub fn finance_fixed_decimal_type() -> StructuredInfoType {
    crate::FinanceFixedDecimal::semantic_type().expect("checked native finance fixed-decimal Type")
}

pub fn finance_currency_type() -> StructuredInfoType {
    crate::FinanceCurrency::semantic_type().expect("checked native finance currency Type")
}

pub fn finance_money_type() -> StructuredInfoType {
    crate::FinanceMoney::semantic_type().expect("checked native finance money Type")
}

pub fn finance_instrument_type() -> StructuredInfoType {
    crate::FinanceCurrencyPair::semantic_type().expect("checked native finance currency-pair Type")
}

pub fn finance_instant_type() -> StructuredInfoType {
    crate::FinanceObservedInstant::semantic_type()
        .expect("checked native finance observed-instant Type")
}

pub fn finance_freshness_type() -> StructuredInfoType {
    crate::FinanceQuoteFreshness::semantic_type()
        .expect("checked native finance quote-freshness Type")
}

pub fn finance_quote_type() -> StructuredInfoType {
    crate::FinanceQuote::semantic_type().expect("checked native finance quote Type")
}

pub fn finance_rate_type() -> StructuredInfoType {
    crate::FinanceRateObservation::semantic_type()
        .expect("checked native finance rate-observation Type")
}

pub fn finance_transaction_event_type() -> StructuredInfoType {
    crate::FinanceTransactionEvent::semantic_type()
        .expect("checked native finance transaction-event Type")
}

pub fn finance_transaction_events_type() -> StructuredInfoType {
    crate::FinanceTransactionEventsThree::semantic_type()
        .expect("checked native finance transaction-events Type")
}

pub fn finance_money_comparison_type() -> StructuredInfoType {
    crate::FinanceMoneyComparison::semantic_type().expect("checked native finance comparison Type")
}
