//! Deterministic reference finance fixtures and structured exact-money operations.

use alloc::string::ToString;
use conduit_core::{Quantity, QuantityUnit, StructuredInfoValue};
use conduit_plot::rust_binding::NativeRustBinding;
use core::cmp::Ordering;

use crate::finance::*;
use crate::{
    FinanceCurrencyPair, FinanceMoneyComparison, FinanceObservedInstant, FinanceOrderIdentity,
    FinanceQuote, FinanceQuoteFreshness, FinanceQuoteSource, FinanceRejectionReason,
    FinanceTransactionEvent, FinanceTransactionEventsThree,
};

pub struct FinanceFixture {
    pub convertible: StructuredInfoValue,
    pub left: StructuredInfoValue,
    pub rate: StructuredInfoValue,
    pub right: StructuredInfoValue,
    pub quote: StructuredInfoValue,
    pub events: StructuredInfoValue,
}

pub fn deterministic_finance_fixture() -> Result<FinanceFixture, FinanceRefusal> {
    let left = Money::new(FixedDecimal::new(1_234, 2)?, Currency::Usd)?;
    let right = Money::new(FixedDecimal::new(66, 2)?, Currency::Usd)?;
    Ok(FinanceFixture {
        convertible: money_value(Money::new(FixedDecimal::new(1_000, 2)?, Currency::Eur)?)?,
        left: money_value(left)?,
        rate: rate_observation_value(&deterministic_rate_observation()?)?,
        right: money_value(right)?,
        quote: deterministic_quote()?,
        events: deterministic_transaction_events()?,
    })
}

pub fn add_money_values(
    left: &StructuredInfoValue,
    right: &StructuredInfoValue,
) -> Result<StructuredInfoValue, FinanceRefusal> {
    money_value(add_money(
        decode_money_value(left)?,
        decode_money_value(right)?,
    )?)
}

pub fn compare_money_values(
    left: &StructuredInfoValue,
    right: &StructuredInfoValue,
) -> Result<StructuredInfoValue, FinanceRefusal> {
    let comparison = compare_money(decode_money_value(left)?, decode_money_value(right)?)?;
    Ok(match comparison {
        Ordering::Less => FinanceMoneyComparison::less(),
        Ordering::Equal => FinanceMoneyComparison::equal(),
        Ordering::Greater => FinanceMoneyComparison::greater(),
    }
    .into_structured()?)
}

pub fn convert_money_values(
    money: &StructuredInfoValue,
    rate: &StructuredInfoValue,
) -> Result<StructuredInfoValue, FinanceRefusal> {
    let observation = RateObservation::from_structured(rate.clone())?;
    money_value(convert_money(decode_money_value(money)?, &observation)?)
}

pub fn decode_money_value(value: &StructuredInfoValue) -> Result<Money, FinanceRefusal> {
    Ok(Money::from_structured(value.clone())?)
}

pub fn deterministic_rate_observation() -> Result<RateObservation, FinanceRefusal> {
    Ok(RateObservation::new(
        Currency::Eur,
        1_788_000_000,
        crate::FinanceRateProfile::new("finance/exact-decimal-rate@1".into())?,
        Currency::Usd,
        FixedDecimal::new(108_250, 5)?,
        crate::FinanceRateSource::new("fixture/ecb-reference".into())?,
    )?)
}

fn rate_observation_value(
    observation: &RateObservation,
) -> Result<StructuredInfoValue, FinanceRefusal> {
    Ok(observation.clone().into_structured()?)
}

fn deterministic_quote() -> Result<StructuredInfoValue, FinanceRefusal> {
    let observed = FinanceObservedInstant::new(1_788_000_000)?;
    let reference = FinanceObservedInstant::new(1_788_000_120)?;
    Ok(FinanceQuote::new(
        Money::new(FixedDecimal::new(108_270, 5)?, Currency::Usd)?,
        Money::new(FixedDecimal::new(108_250, 5)?, Currency::Usd)?,
        FinanceQuoteFreshness::stale(Quantity::new(120, QuantityUnit::Second), reference)?,
        FinanceCurrencyPair::new(Currency::Eur, Currency::Usd)?,
        observed,
        FinanceQuoteSource::new("fixture/eur-usd".to_string())?,
    )?
    .into_structured()?)
}

fn deterministic_transaction_events() -> Result<StructuredInfoValue, FinanceRefusal> {
    let amount = Money::new(FixedDecimal::new(10_000, 2)?, Currency::Eur)?;
    let placed = FinanceTransactionEvent::placed(
        amount.clone(),
        FinanceObservedInstant::new(1_788_000_001)?,
        FinanceOrderIdentity::new("fixture/order-1".to_string())?,
    )?;
    let filled = FinanceTransactionEvent::filled(
        amount,
        FinanceObservedInstant::new(1_788_000_002)?,
        FinanceOrderIdentity::new("fixture/order-1".to_string())?,
        Money::new(FixedDecimal::new(108_260, 5)?, Currency::Usd)?,
    )?;
    let rejected = FinanceTransactionEvent::rejected(
        FinanceObservedInstant::new(1_788_000_003)?,
        FinanceOrderIdentity::new("fixture/order-2".to_string())?,
        FinanceRejectionReason::new("fixture/limit-refused".to_string())?,
    )?;
    Ok(FinanceTransactionEventsThree::new([placed, filled, rejected])?.into_structured()?)
}

fn money_value(money: Money) -> Result<StructuredInfoValue, FinanceRefusal> {
    Ok(money.into_structured()?)
}
