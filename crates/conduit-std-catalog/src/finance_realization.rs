//! Deterministic finance fixtures and structured exact-money operations.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    Quantity, QuantityUnit, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use core::cmp::Ordering;

use super::finance::*;

pub struct FinanceFixture {
    pub convertible: StructuredInfoValue,
    pub left: StructuredInfoValue,
    pub rate: StructuredInfoValue,
    pub right: StructuredInfoValue,
    pub quote: StructuredInfoValue,
    pub events: StructuredInfoValue,
}

pub fn deterministic_finance_fixture() -> Result<FinanceFixture, FinanceRefusal> {
    let left = Money {
        amount: FixedDecimal::new(1_234, 2)?,
        currency: Currency::Usd,
    };
    let right = Money {
        amount: FixedDecimal::new(66, 2)?,
        currency: Currency::Usd,
    };
    Ok(FinanceFixture {
        convertible: money_value(Money {
            amount: FixedDecimal::new(1_000, 2)?,
            currency: Currency::Eur,
        })?,
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
    unit_variant(
        finance_money_comparison_type(),
        match comparison {
            Ordering::Less => "less",
            Ordering::Equal => "equal",
            Ordering::Greater => "greater",
        },
    )
}

pub fn convert_money_values(
    money: &StructuredInfoValue,
    rate: &StructuredInfoValue,
) -> Result<StructuredInfoValue, FinanceRefusal> {
    if rate.value_type() != &finance_rate_type() {
        return Err(FinanceRefusal::MalformedInfo);
    }
    let instrument = record_field(rate, "instrument")?;
    let observation = RateObservation {
        base: Currency::from_tag(variant_tag(record_field(instrument, "base")?)?)?,
        quote: Currency::from_tag(variant_tag(record_field(instrument, "quote")?)?)?,
        rate: FixedDecimal::decode(leaf_bytes(record_field(rate, "rate")?)?)?,
        observed_ticks: parse_count(record_field(record_field(rate, "observed_at")?, "ticks")?)?,
        source: leaf_text(record_field(rate, "source")?)?,
        profile: leaf_text(record_field(rate, "profile")?)?,
    };
    money_value(convert_money(decode_money_value(money)?, &observation)?)
}

pub fn decode_money_value(value: &StructuredInfoValue) -> Result<Money, FinanceRefusal> {
    if value.value_type() != &finance_money_type() {
        return Err(FinanceRefusal::MalformedInfo);
    }
    let amount = leaf_bytes(record_field(value, "amount")?)?;
    let currency = variant_tag(record_field(value, "currency")?)?;
    Ok(Money {
        amount: FixedDecimal::decode(amount)?,
        currency: Currency::from_tag(currency)?,
    })
}

pub fn deterministic_rate_observation() -> Result<RateObservation<'static>, FinanceRefusal> {
    Ok(RateObservation {
        base: Currency::Eur,
        quote: Currency::Usd,
        rate: FixedDecimal::new(108_250, 5)?,
        observed_ticks: 1_788_000_000,
        source: "fixture/ecb-reference",
        profile: "finance/exact-decimal-rate@1",
    })
}

fn rate_observation_value(
    observation: &RateObservation<'_>,
) -> Result<StructuredInfoValue, FinanceRefusal> {
    record_value(
        finance_rate_type(),
        vec![
            (
                "instrument",
                instrument_value(observation.base, observation.quote)?,
            ),
            ("observed_at", instant_value(observation.observed_ticks)?),
            ("profile", text_value(observation.profile)),
            (
                "rate",
                StructuredInfoValue::leaf(
                    finance_fixed_decimal_type(),
                    observation.rate.encode().to_vec(),
                )?,
            ),
            ("source", text_value(observation.source)),
        ],
    )
}

fn deterministic_quote() -> Result<StructuredInfoValue, FinanceRefusal> {
    let observed = instant_value(1_788_000_000)?;
    let reference = instant_value(1_788_000_120)?;
    let freshness = StructuredInfoValue::variant(
        finance_freshness_type(),
        "stale",
        record_value(
            freshness_payload_type()?,
            vec![
                (
                    "age",
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(conduit_core::kind_id(
                            conduit_core::QUANTITY_INFO_ID,
                        ))?,
                        Quantity::new(120, QuantityUnit::Second).encode().to_vec(),
                    )?,
                ),
                ("reference", reference),
            ],
        )?,
    )?;
    record_value(
        finance_quote_type(),
        vec![
            (
                "ask",
                money_value(Money {
                    amount: FixedDecimal::new(108_270, 5)?,
                    currency: Currency::Usd,
                })?,
            ),
            (
                "bid",
                money_value(Money {
                    amount: FixedDecimal::new(108_250, 5)?,
                    currency: Currency::Usd,
                })?,
            ),
            ("freshness", freshness),
            (
                "instrument",
                instrument_value(Currency::Eur, Currency::Usd)?,
            ),
            ("observed_at", observed),
            ("source", text_value("fixture/eur-usd")),
        ],
    )
}

fn deterministic_transaction_events() -> Result<StructuredInfoValue, FinanceRefusal> {
    let event_type = finance_transaction_event_type();
    let amount = Money {
        amount: FixedDecimal::new(10_000, 2)?,
        currency: Currency::Eur,
    };
    let placed = StructuredInfoValue::variant(
        event_type.clone(),
        "placed",
        record_value(
            variant_payload_type(&event_type, "placed")?,
            vec![
                ("amount", money_value(amount)?),
                ("observed_at", instant_value(1_788_000_001)?),
                ("order_id", text_value("fixture/order-1")),
            ],
        )?,
    )?;
    let filled = StructuredInfoValue::variant(
        event_type.clone(),
        "filled",
        record_value(
            variant_payload_type(&event_type, "filled")?,
            vec![
                ("amount", money_value(amount)?),
                ("observed_at", instant_value(1_788_000_002)?),
                ("order_id", text_value("fixture/order-1")),
                (
                    "price",
                    money_value(Money {
                        amount: FixedDecimal::new(108_260, 5)?,
                        currency: Currency::Usd,
                    })?,
                ),
            ],
        )?,
    )?;
    let rejected = StructuredInfoValue::variant(
        event_type.clone(),
        "rejected",
        record_value(
            variant_payload_type(&event_type, "rejected")?,
            vec![
                ("observed_at", instant_value(1_788_000_003)?),
                ("order_id", text_value("fixture/order-2")),
                ("reason", text_value("fixture/limit-refused")),
            ],
        )?,
    )?;
    Ok(StructuredInfoValue::collection(
        finance_transaction_events_type(),
        vec![placed, filled, rejected],
    )?)
}

fn money_value(money: Money) -> Result<StructuredInfoValue, FinanceRefusal> {
    record_value(
        finance_money_type(),
        vec![
            (
                "amount",
                StructuredInfoValue::leaf(
                    finance_fixed_decimal_type(),
                    money.amount.encode().to_vec(),
                )?,
            ),
            (
                "currency",
                unit_variant(finance_currency_type(), money.currency.tag())?,
            ),
        ],
    )
}

fn instrument_value(
    base: Currency,
    quote: Currency,
) -> Result<StructuredInfoValue, FinanceRefusal> {
    record_value(
        finance_instrument_type(),
        vec![
            ("base", unit_variant(finance_currency_type(), base.tag())?),
            ("quote", unit_variant(finance_currency_type(), quote.tag())?),
        ],
    )
}

fn instant_value(ticks: u64) -> Result<StructuredInfoValue, FinanceRefusal> {
    record_value(
        finance_instant_type(),
        vec![
            ("basis", text_value("unix/utc@1")),
            ("resolution_ticks", count_value(1)),
            (
                "scale",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(conduit_core::kind_id("time/scale@1"))?,
                    b"seconds".to_vec(),
                )?,
            ),
            ("ticks", count_value(ticks)),
            ("uncertainty_ticks", count_value(0)),
        ],
    )
}

fn freshness_payload_type() -> Result<StructuredInfoType, FinanceRefusal> {
    variant_payload_type(&finance_freshness_type(), "stale")
}

fn variant_payload_type(
    value_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, FinanceRefusal> {
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return Err(FinanceRefusal::MalformedInfo);
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or(FinanceRefusal::MalformedInfo)
}

fn unit_variant(
    value_type: StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoValue, FinanceRefusal> {
    Ok(StructuredInfoValue::variant(
        value_type,
        tag,
        StructuredInfoValue::leaf(finance_unit_type(), Vec::new())?,
    )?)
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, FinanceRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}

fn text_value(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/text@1")).unwrap(),
        value.as_bytes().to_vec(),
    )
    .expect("bounded fixture text")
}

fn count_value(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/count@1")).unwrap(),
        value.to_string().into_bytes(),
    )
    .expect("bounded fixture count")
}

fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, FinanceRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(FinanceRefusal::MalformedInfo);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(FinanceRefusal::MalformedInfo)
}

fn leaf_bytes(value: &StructuredInfoValue) -> Result<&[u8], FinanceRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(FinanceRefusal::MalformedInfo);
    };
    Ok(bytes)
}

fn leaf_text(value: &StructuredInfoValue) -> Result<&str, FinanceRefusal> {
    core::str::from_utf8(leaf_bytes(value)?).map_err(|_| FinanceRefusal::MalformedInfo)
}

fn parse_count(value: &StructuredInfoValue) -> Result<u64, FinanceRefusal> {
    leaf_text(value)?
        .parse()
        .map_err(|_| FinanceRefusal::MalformedInfo)
}

fn variant_tag(value: &StructuredInfoValue) -> Result<&str, FinanceRefusal> {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        return Err(FinanceRefusal::MalformedInfo);
    };
    Ok(tag)
}
