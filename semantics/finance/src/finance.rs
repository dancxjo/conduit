//! Host-neutral exact finite monetary semantics without floats or provider symbols.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredInfoRefusal, StructuredInfoType, StructuredVariantCase,
    QUANTITY_INFO_ID,
};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    pub amount: FixedDecimal,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateObservation<'a> {
    pub base: Currency,
    pub quote: Currency,
    pub rate: FixedDecimal,
    pub observed_ticks: u64,
    pub source: &'a str,
    pub profile: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinanceRefusal {
    CurrencyMismatch { left: Currency, right: Currency },
    RatePairMismatch,
    Overflow,
    InvalidObservation,
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
    require_same_currency(&left.currency, &right.currency)?;
    Ok(Money {
        amount: left.amount.checked_add(&right.amount)?,
        currency: left.currency.clone(),
    })
}

pub fn compare_money(left: Money, right: Money) -> Result<Ordering, FinanceRefusal> {
    require_same_currency(&left.currency, &right.currency)?;
    left.amount.checked_cmp(&right.amount)
}

pub fn convert_money(money: Money, rate: &RateObservation<'_>) -> Result<Money, FinanceRefusal> {
    if rate.base == rate.quote || money.currency != rate.base {
        return Err(FinanceRefusal::RatePairMismatch);
    }
    if rate.source.is_empty() || rate.profile.is_empty() {
        return Err(FinanceRefusal::InvalidObservation);
    }
    Ok(Money {
        amount: money.amount.checked_mul(&rate.rate)?,
        currency: rate.quote.clone(),
    })
}

fn require_same_currency(left: &Currency, right: &Currency) -> Result<(), FinanceRefusal> {
    if left != right {
        return Err(FinanceRefusal::CurrencyMismatch {
            left: left.clone(),
            right: right.clone(),
        });
    }
    Ok(())
}

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed finance leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed finance field")
}

fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed finance case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed finance record")
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit")
}

pub fn finance_fixed_decimal_type() -> StructuredInfoType {
    crate::FinanceFixedDecimal::semantic_type().expect("checked native finance fixed-decimal Type")
}

pub fn finance_currency_type() -> StructuredInfoType {
    crate::FinanceCurrency::semantic_type().expect("checked native finance currency Type")
}

pub fn finance_money_type() -> StructuredInfoType {
    record(
        "finance/money@1",
        vec![
            field("amount", finance_fixed_decimal_type()),
            field("currency", finance_currency_type()),
        ],
    )
}

pub fn finance_instrument_type() -> StructuredInfoType {
    crate::FinanceCurrencyPair::semantic_type().expect("checked native finance currency-pair Type")
}

pub fn finance_instant_type() -> StructuredInfoType {
    record(
        "finance/observed-instant@1",
        vec![
            field("basis", leaf("value/text")),
            field("resolution_ticks", leaf("value/count")),
            field("scale", leaf("time/scale@1")),
            field("ticks", leaf("value/count")),
            field("uncertainty_ticks", leaf("value/count")),
        ],
    )
}

fn freshness_detail_type() -> StructuredInfoType {
    record(
        "finance/quote-freshness-detail@1",
        vec![
            field("age", leaf(QUANTITY_INFO_ID)),
            field("reference", finance_instant_type()),
        ],
    )
}

pub fn finance_freshness_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("finance/quote-freshness@1"),
        vec![
            case("fresh", freshness_detail_type()),
            case("stale", freshness_detail_type()),
        ],
    )
    .expect("reviewed quote freshness")
}

pub fn finance_quote_type() -> StructuredInfoType {
    record(
        "finance/quote@1",
        vec![
            field("ask", finance_money_type()),
            field("bid", finance_money_type()),
            field("freshness", finance_freshness_type()),
            field("instrument", finance_instrument_type()),
            field("observed_at", finance_instant_type()),
            field("source", leaf("value/text")),
        ],
    )
}

pub fn finance_rate_type() -> StructuredInfoType {
    record(
        "finance/rate-observation@1",
        vec![
            field("instrument", finance_instrument_type()),
            field("observed_at", finance_instant_type()),
            field("profile", leaf("value/text")),
            field("rate", finance_fixed_decimal_type()),
            field("source", leaf("value/text")),
        ],
    )
}

pub fn finance_transaction_event_type() -> StructuredInfoType {
    let placed = record(
        "finance/transaction-placed@1",
        vec![
            field("amount", finance_money_type()),
            field("observed_at", finance_instant_type()),
            field("order_id", leaf("value/text")),
        ],
    );
    let filled = record(
        "finance/transaction-filled@1",
        vec![
            field("amount", finance_money_type()),
            field("observed_at", finance_instant_type()),
            field("order_id", leaf("value/text")),
            field("price", finance_money_type()),
        ],
    );
    let rejected = record(
        "finance/transaction-rejected@1",
        vec![
            field("observed_at", finance_instant_type()),
            field("order_id", leaf("value/text")),
            field("reason", leaf("value/text")),
        ],
    );
    StructuredInfoType::variant(
        kind_id("finance/transaction-event@1"),
        vec![
            case("filled", filled),
            case("placed", placed),
            case("rejected", rejected),
        ],
    )
    .expect("reviewed transaction variants")
}

pub fn finance_transaction_events_type() -> StructuredInfoType {
    StructuredInfoType::collection(
        finance_transaction_event_type(),
        Some(FINANCE_TRANSACTION_EVENT_COUNT),
    )
    .expect("three deterministic transaction events")
}

pub fn finance_money_comparison_type() -> StructuredInfoType {
    crate::FinanceMoneyComparison::semantic_type().expect("checked native finance comparison Type")
}

pub(crate) fn finance_unit_type() -> StructuredInfoType {
    unit_type()
}
