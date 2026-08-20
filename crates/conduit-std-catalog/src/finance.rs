//! Exact finite monetary semantics without floats or provider symbols.

use alloc::{vec, vec::Vec};
use core::cmp::Ordering;
use conduit_core::{
    kind_id, StructuredFieldType, StructuredInfoRefusal, StructuredInfoType,
    StructuredVariantCase, QUANTITY_INFO_ID,
};

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
pub const FINANCE_FIXED_DECIMAL_INFO_ID: &str = "finance/fixed-decimal@1";
pub const FINANCE_MAXIMUM_DECIMAL_SCALE: u8 = 9;
pub const FINANCE_TRANSACTION_EVENT_COUNT: u16 = 3;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedDecimal {
    coefficient: i64,
    scale: u8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Currency {
    Eur,
    Gbp,
    Usd,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    ScaleOutOfRange { maximum: u8, actual: u8 },
    Overflow,
    InvalidObservation,
    MalformedInfo,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for FinanceRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

impl FixedDecimal {
    pub fn new(coefficient: i64, scale: u8) -> Result<Self, FinanceRefusal> {
        if scale > FINANCE_MAXIMUM_DECIMAL_SCALE {
            return Err(FinanceRefusal::ScaleOutOfRange {
                maximum: FINANCE_MAXIMUM_DECIMAL_SCALE,
                actual: scale,
            });
        }
        Ok(Self { coefficient, scale })
    }

    pub const fn coefficient(self) -> i64 {
        self.coefficient
    }

    pub const fn scale(self) -> u8 {
        self.scale
    }

    pub const fn encode(self) -> [u8; 9] {
        let coefficient = self.coefficient.to_le_bytes();
        [
            self.scale,
            coefficient[0],
            coefficient[1],
            coefficient[2],
            coefficient[3],
            coefficient[4],
            coefficient[5],
            coefficient[6],
            coefficient[7],
        ]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FinanceRefusal> {
        if encoded.len() != 9 {
            return Err(FinanceRefusal::MalformedInfo);
        }
        let coefficient = i64::from_le_bytes(
            encoded[1..]
                .try_into()
                .expect("fixed decimal length checked before decode"),
        );
        Self::new(coefficient, encoded[0])
    }

    pub fn checked_add(self, other: Self) -> Result<Self, FinanceRefusal> {
        let scale = self.scale.max(other.scale);
        let left = self.at_scale(scale)?;
        let right = other.at_scale(scale)?;
        Self::new(
            left.checked_add(right).ok_or(FinanceRefusal::Overflow)?,
            scale,
        )
    }

    pub fn checked_cmp(self, other: Self) -> Result<Ordering, FinanceRefusal> {
        let scale = self.scale.max(other.scale);
        Ok(self.at_scale(scale)?.cmp(&other.at_scale(scale)?))
    }

    pub fn checked_mul(self, other: Self) -> Result<Self, FinanceRefusal> {
        let scale = self
            .scale
            .checked_add(other.scale)
            .ok_or(FinanceRefusal::Overflow)?;
        Self::new(
            self.coefficient
                .checked_mul(other.coefficient)
                .ok_or(FinanceRefusal::Overflow)?,
            scale,
        )
    }

    fn at_scale(self, scale: u8) -> Result<i64, FinanceRefusal> {
        let factor = 10_i64
            .checked_pow(u32::from(scale - self.scale))
            .ok_or(FinanceRefusal::Overflow)?;
        self.coefficient
            .checked_mul(factor)
            .ok_or(FinanceRefusal::Overflow)
    }
}

impl Currency {
    pub const fn tag(self) -> &'static str {
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
    require_same_currency(left.currency, right.currency)?;
    Ok(Money {
        amount: left.amount.checked_add(right.amount)?,
        currency: left.currency,
    })
}

pub fn compare_money(left: Money, right: Money) -> Result<Ordering, FinanceRefusal> {
    require_same_currency(left.currency, right.currency)?;
    left.amount.checked_cmp(right.amount)
}

pub fn convert_money(money: Money, rate: &RateObservation<'_>) -> Result<Money, FinanceRefusal> {
    if rate.base == rate.quote || money.currency != rate.base {
        return Err(FinanceRefusal::RatePairMismatch);
    }
    if rate.source.is_empty() || rate.profile.is_empty() {
        return Err(FinanceRefusal::InvalidObservation);
    }
    Ok(Money {
        amount: money.amount.checked_mul(rate.rate)?,
        currency: rate.quote,
    })
}

fn require_same_currency(left: Currency, right: Currency) -> Result<(), FinanceRefusal> {
    if left != right {
        return Err(FinanceRefusal::CurrencyMismatch { left, right });
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
    leaf("value/unit@1")
}

pub fn finance_fixed_decimal_type() -> StructuredInfoType {
    leaf(FINANCE_FIXED_DECIMAL_INFO_ID)
}

pub fn finance_currency_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("finance/currency@1"),
        vec![
            case("eur", unit_type()),
            case("gbp", unit_type()),
            case("usd", unit_type()),
        ],
    )
    .expect("reviewed currency identity")
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
    record(
        "finance/currency-pair@1",
        vec![
            field("base", finance_currency_type()),
            field("quote", finance_currency_type()),
        ],
    )
}

pub fn finance_instant_type() -> StructuredInfoType {
    record(
        "finance/observed-instant@1",
        vec![
            field("basis", leaf("value/text@1")),
            field("resolution_ticks", leaf("value/count@1")),
            field("scale", leaf("time/scale@1")),
            field("ticks", leaf("value/count@1")),
            field("uncertainty_ticks", leaf("value/count@1")),
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
            field("source", leaf("value/text@1")),
        ],
    )
}

pub fn finance_rate_type() -> StructuredInfoType {
    record(
        "finance/rate-observation@1",
        vec![
            field("instrument", finance_instrument_type()),
            field("observed_at", finance_instant_type()),
            field("profile", leaf("value/text@1")),
            field("rate", finance_fixed_decimal_type()),
            field("source", leaf("value/text@1")),
        ],
    )
}

pub fn finance_transaction_event_type() -> StructuredInfoType {
    let placed = record(
        "finance/transaction-placed@1",
        vec![
            field("amount", finance_money_type()),
            field("observed_at", finance_instant_type()),
            field("order_id", leaf("value/text@1")),
        ],
    );
    let filled = record(
        "finance/transaction-filled@1",
        vec![
            field("amount", finance_money_type()),
            field("observed_at", finance_instant_type()),
            field("order_id", leaf("value/text@1")),
            field("price", finance_money_type()),
        ],
    );
    let rejected = record(
        "finance/transaction-rejected@1",
        vec![
            field("observed_at", finance_instant_type()),
            field("order_id", leaf("value/text@1")),
            field("reason", leaf("value/text@1")),
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
    StructuredInfoType::variant(
        kind_id("finance/money-comparison@1"),
        vec![
            case("equal", unit_type()),
            case("greater", unit_type()),
            case("less", unit_type()),
        ],
    )
    .expect("reviewed money comparison")
}

pub(crate) fn finance_unit_type() -> StructuredInfoType {
    unit_type()
}
