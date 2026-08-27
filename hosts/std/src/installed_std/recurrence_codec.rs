//! Exact conversion between structured recurrence Info and core temporal semantics.

use conduit_core::{
    BootId, HostId, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_time::{
    CivilFoldPolicy, CivilGapPolicy, CivilOccurrenceResolution, CivilResolutionChoice,
    CivilResolutionPolicy, LocalDate, LocalDateTime, LocalTime, MonotonicClockIdentity,
    MonotonicDuration, MonotonicInstant, NamedTimeZone, OccurrenceInstant, RecurrenceDefinition,
    RecurrenceExpansion, RecurrenceRule, RecurrenceUntil, RecurrenceWindow, TemporalInstant,
    TemporalScale, WeekdaySet, ZonedResolution,
};

pub(super) struct DecodedRecurrence {
    pub(super) definition: RecurrenceDefinition,
    pub(super) expansion: RecurrenceExpansion,
    pub(super) resolutions: Vec<CivilOccurrenceResolution>,
    pub(super) policy: CivilResolutionPolicy,
}

pub(super) fn decode(value: &StructuredInfoValue) -> Result<DecodedRecurrence, String> {
    if value.value_type() != &conduit_std_catalog::recurrence_request_type() {
        return Err("recurrence request type differs from the installed contract".into());
    }
    let fields = record(value)?;
    let definition = RecurrenceDefinition {
        identity: text(field(fields, "identity")?)?,
        rule: rule(field(fields, "rule")?)?,
        maximum_occurrences: u32_value(field(fields, "maximum_occurrences")?)?,
        until: until(field(fields, "until")?)?,
        excluded_ordinals: slots(field(fields, "excluded_ordinals")?, "exclude")?
            .map(|(_, payload)| u32_value(payload))
            .collect::<Result<_, _>>()?,
    };
    let expansion = RecurrenceExpansion {
        maximum_results: u32_value(field(fields, "maximum_results")?)?,
        window: window(field(fields, "window")?)?,
    };
    let resolutions = slots(field(fields, "resolutions")?, "resolution")?
        .map(resolution_payload)
        .collect::<Result<Vec<_>, _>>()?;
    let policy = CivilResolutionPolicy {
        gap: match text(field(fields, "gap_policy")?)?.as_str() {
            "skip" => CivilGapPolicy::Skip,
            "use_before" => CivilGapPolicy::UseBefore,
            "use_after" => CivilGapPolicy::UseAfter,
            "refuse" => CivilGapPolicy::Refuse,
            _ => return Err("unknown recurrence gap policy".into()),
        },
        fold: match text(field(fields, "fold_policy")?)?.as_str() {
            "earlier" => CivilFoldPolicy::Earlier,
            "later" => CivilFoldPolicy::Later,
            "both" => CivilFoldPolicy::Both,
            "refuse" => CivilFoldPolicy::Refuse,
            _ => return Err("unknown recurrence fold policy".into()),
        },
    };
    definition
        .validate()
        .map_err(|error| format!("recurrence definition refusal: {error:?}"))?;
    expansion
        .validate()
        .map_err(|error| format!("recurrence expansion refusal: {error:?}"))?;
    Ok(DecodedRecurrence {
        definition,
        expansion,
        resolutions,
        policy,
    })
}

fn rule(value: &StructuredInfoValue) -> Result<RecurrenceRule, String> {
    let (tag, payload) = variant(value)?;
    let fields = record(payload)?;
    match tag {
        "one_shot" => Ok(RecurrenceRule::OneShot {
            at: instant(field(fields, "at")?)?,
        }),
        "fixed_elapsed" => {
            let first = monotonic(field(fields, "first")?)?;
            Ok(RecurrenceRule::FixedElapsed {
                every: MonotonicDuration::new(
                    count_value(field(fields, "every_ticks")?)?,
                    first.clock().scale(),
                ),
                first,
            })
        }
        "civil_weekdays" => {
            let zone = NamedTimeZone::new(
                text(field(fields, "zone")?)?,
                text(field(fields, "rule_set")?)?,
            )
            .map_err(|error| format!("recurrence zone refusal: {error:?}"))?;
            Ok(RecurrenceRule::CivilWeekdays {
                first_date: date(field(fields, "first_date")?)?,
                local_time: time(field(fields, "local_time")?)?,
                zone,
                weekdays: weekdays(count_value(field(fields, "weekdays")?)?)?,
                excluded_dates: slots(field(fields, "excluded_dates")?, "exclude")?
                    .map(|(_, payload)| date(payload))
                    .collect::<Result<_, _>>()?,
            })
        }
        _ => Err("unknown recurrence rule".into()),
    }
}

fn until(value: &StructuredInfoValue) -> Result<Option<RecurrenceUntil>, String> {
    let (tag, payload) = variant(value)?;
    match tag {
        "none" => Ok(None),
        "wall" => Ok(Some(RecurrenceUntil::Wall(instant(payload)?))),
        "monotonic" => Ok(Some(RecurrenceUntil::Monotonic(monotonic(payload)?))),
        "civil_date" => Ok(Some(RecurrenceUntil::CivilDate(date(payload)?))),
        _ => Err("unknown recurrence until boundary".into()),
    }
}

fn window(value: &StructuredInfoValue) -> Result<RecurrenceWindow, String> {
    let (tag, payload) = variant(value)?;
    let fields = record(payload)?;
    match tag {
        "wall" => Ok(RecurrenceWindow::Wall {
            start: instant(field(fields, "start")?)?,
            end: instant(field(fields, "end")?)?,
        }),
        "monotonic" => Ok(RecurrenceWindow::Monotonic {
            start: monotonic(field(fields, "start")?)?,
            end: monotonic(field(fields, "end")?)?,
        }),
        _ => Err("unknown recurrence window".into()),
    }
}

fn resolution_payload(
    tagged: (&str, &StructuredInfoValue),
) -> Result<CivilOccurrenceResolution, String> {
    let (tag, payload) = tagged;
    let fields = record(payload)?;
    let local = LocalDateTime::new(
        date(field(fields, "local_date")?)?,
        time(field(fields, "local_time")?)?,
    );
    let zone = NamedTimeZone::new(
        text(field(fields, "zone")?)?,
        text(field(fields, "rule_set")?)?,
    )
    .map_err(|error| format!("civil resolution zone refusal: {error:?}"))?;
    let resolution = match tag {
        "unique" => ZonedResolution::Unique {
            local,
            zone,
            instant: instant(field(fields, "instant")?)?,
        },
        "ambiguous" => ZonedResolution::Ambiguous {
            local,
            zone,
            earlier: instant(field(fields, "earlier")?)?,
            later: instant(field(fields, "later")?)?,
        },
        "nonexistent" => ZonedResolution::Nonexistent {
            local,
            zone,
            gap_before: instant(field(fields, "gap_before")?)?,
            gap_after: instant(field(fields, "gap_after")?)?,
        },
        _ => return Err("unknown civil occurrence resolution".into()),
    };
    Ok(CivilOccurrenceResolution {
        ordinal: u32_value(field(fields, "ordinal")?)?,
        resolution,
    })
}

fn slots<'a>(
    value: &'a StructuredInfoValue,
    active_tag: &'static str,
) -> Result<impl Iterator<Item = (&'a str, &'a StructuredInfoValue)>, String> {
    let slots = collection(value)?;
    let parsed = slots.iter().map(variant).collect::<Result<Vec<_>, _>>()?;
    let mut unused_seen = false;
    for (tag, _) in &parsed {
        match *tag {
            "unused" => unused_seen = true,
            tag if tag == active_tag
                || (active_tag == "resolution"
                    && matches!(tag, "unique" | "ambiguous" | "nonexistent")) =>
            {
                if unused_seen {
                    return Err("active recurrence slot follows an unused slot".into());
                }
            }
            _ => return Err("unknown recurrence slot tag".into()),
        }
    }
    Ok(parsed.into_iter().filter(|(tag, _)| *tag != "unused"))
}

fn instant(value: &StructuredInfoValue) -> Result<TemporalInstant, String> {
    let fields = record(value)?;
    Ok(TemporalInstant {
        ticks: count_value(field(fields, "ticks")?)?,
        scale: scale(field(fields, "scale")?)?,
        clock_basis: text(field(fields, "basis")?)?,
        resolution_ticks: count_value(field(fields, "resolution_ticks")?)?,
        uncertainty_ticks: 0,
    })
}

fn monotonic(value: &StructuredInfoValue) -> Result<MonotonicInstant, String> {
    let fields = record(value)?;
    let clock = MonotonicClockIdentity::new(
        HostId::from(text(field(fields, "host")?)?),
        BootId::from(text(field(fields, "boot")?)?),
        text(field(fields, "basis")?)?,
        scale(field(fields, "scale")?)?,
        count_value(field(fields, "resolution_ticks")?)?,
        count_value(field(fields, "uncertainty_ticks")?)?,
    )
    .map_err(|error| format!("monotonic clock refusal: {error:?}"))?;
    MonotonicInstant::new(count_value(field(fields, "ticks")?)?, clock)
        .map_err(|error| format!("monotonic instant refusal: {error:?}"))
}

pub(super) fn occurrence_instant(value: &OccurrenceInstant) -> Result<StructuredInfoValue, String> {
    let value_type = conduit_std_catalog::recurrence_occurrence_instant_type();
    let (tag, payload) = match value {
        OccurrenceInstant::Wall(value) => ("wall", instant_value(value)?),
        OccurrenceInstant::Monotonic(value) => ("monotonic", monotonic_value(value)?),
        OccurrenceInstant::Civil {
            local,
            zone,
            instant,
            resolution,
        } => {
            let payload_type = match &value_type.shape() {
                conduit_core::StructuredInfoTypeShape::Variant { cases, .. } => cases
                    .iter()
                    .find(|case| case.tag() == "civil")
                    .unwrap()
                    .payload_type()
                    .clone(),
                _ => unreachable!(),
            };
            let payload = StructuredInfoValue::record(
                payload_type,
                vec![
                    value_field("instant", instant_value(instant)?),
                    value_field(
                        "local_date",
                        leaf("time/local-date@1", &format_date(local.date))?,
                    ),
                    value_field(
                        "local_time",
                        leaf("time/local-time@1", &format_time(local.time))?,
                    ),
                    value_field(
                        "resolution",
                        leaf(
                            "time/civil-resolution-choice@1",
                            resolution_name(*resolution),
                        )?,
                    ),
                    value_field("rule_set", leaf("value/text@1", zone.rule_set())?),
                    value_field("zone", leaf("value/text@1", zone.identity())?),
                ],
            )
            .map_err(structured)?;
            ("civil", payload)
        }
    };
    StructuredInfoValue::variant(value_type, tag, payload).map_err(structured)
}

fn instant_value(value: &TemporalInstant) -> Result<StructuredInfoValue, String> {
    StructuredInfoValue::record(
        conduit_std_catalog::recurrence_instant_type(),
        vec![
            value_field("basis", leaf("value/text@1", &value.clock_basis)?),
            value_field("resolution_ticks", count(value.resolution_ticks)?),
            value_field("scale", leaf("time/scale@1", scale_name(value.scale))?),
            value_field("ticks", count(value.ticks)?),
        ],
    )
    .map_err(structured)
}

fn monotonic_value(value: &MonotonicInstant) -> Result<StructuredInfoValue, String> {
    let clock = value.clock();
    StructuredInfoValue::record(
        conduit_std_catalog::recurrence_monotonic_type(),
        vec![
            value_field("basis", leaf("value/text@1", clock.basis_id())?),
            value_field("boot", leaf("value/text@1", clock.boot_id().as_str())?),
            value_field("host", leaf("value/text@1", clock.host_id().as_str())?),
            value_field("resolution_ticks", count(clock.resolution_ticks())?),
            value_field("scale", leaf("time/scale@1", scale_name(clock.scale()))?),
            value_field("ticks", count(value.ticks())?),
            value_field("uncertainty_ticks", count(clock.uncertainty_ticks())?),
        ],
    )
    .map_err(structured)
}

fn record(value: &StructuredInfoValue) -> Result<&[StructuredFieldValue], String> {
    match value.shape() {
        StructuredInfoValueShape::Record(fields) => Ok(fields),
        _ => Err("recurrence value is not the required record".into()),
    }
}

fn collection(value: &StructuredInfoValue) -> Result<&[StructuredInfoValue], String> {
    match value.shape() {
        StructuredInfoValueShape::Collection(values) => Ok(values),
        _ => Err("recurrence value is not the required collection".into()),
    }
}

fn variant(value: &StructuredInfoValue) -> Result<(&str, &StructuredInfoValue), String> {
    match value.shape() {
        StructuredInfoValueShape::Variant { tag, payload } => Ok((tag, payload)),
        _ => Err("recurrence value is not the required variant".into()),
    }
}

fn field<'a>(
    fields: &'a [StructuredFieldValue],
    name: &str,
) -> Result<&'a StructuredInfoValue, String> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or_else(|| format!("recurrence field '{name}' is missing"))
}

fn text(value: &StructuredInfoValue) -> Result<String, String> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => {
            let raw = core::str::from_utf8(bytes)
                .map_err(|_| "recurrence leaf is not UTF-8".to_string())?;
            if raw.starts_with('"') {
                serde_json::from_str(raw)
                    .map_err(|_| "recurrence quoted leaf is malformed".to_string())
            } else {
                Ok(raw.to_string())
            }
        }
        _ => Err("recurrence value is not a leaf".into()),
    }
}

fn count_value(value: &StructuredInfoValue) -> Result<u64, String> {
    let text = text(value)?;
    let value = text
        .parse::<u64>()
        .map_err(|_| "recurrence count is not canonical".to_string())?;
    (value.to_string() == text)
        .then_some(value)
        .ok_or_else(|| "recurrence count is not canonical".to_string())
}

fn u32_value(value: &StructuredInfoValue) -> Result<u32, String> {
    count_value(value)?
        .try_into()
        .map_err(|_| "recurrence count exceeds u32".into())
}

fn date(value: &StructuredInfoValue) -> Result<LocalDate, String> {
    let text = text(value)?;
    let mut parts = text.split('-');
    let year = parts.next().and_then(|part| part.parse().ok());
    let month = parts.next().and_then(|part| part.parse().ok());
    let day = parts.next().and_then(|part| part.parse().ok());
    if parts.next().is_some() {
        return Err("recurrence date is not YYYY-MM-DD".into());
    }
    LocalDate::new(
        year.ok_or("recurrence date year is invalid")?,
        month.ok_or("recurrence date month is invalid")?,
        day.ok_or("recurrence date day is invalid")?,
    )
    .map_err(|error| format!("recurrence date refusal: {error:?}"))
}

fn time(value: &StructuredInfoValue) -> Result<LocalTime, String> {
    let text = text(value)?;
    let mut parts = text.split(':');
    let hour = parts.next().and_then(|part| part.parse().ok());
    let minute = parts.next().and_then(|part| part.parse().ok());
    let second = parts.next().and_then(|part| part.parse().ok());
    if parts.next().is_some() {
        return Err("recurrence time is not HH:MM:SS".into());
    }
    LocalTime::new(
        hour.ok_or("recurrence time hour is invalid")?,
        minute.ok_or("recurrence time minute is invalid")?,
        second.ok_or("recurrence time second is invalid")?,
        0,
    )
    .map_err(|error| format!("recurrence time refusal: {error:?}"))
}

fn scale(value: &StructuredInfoValue) -> Result<TemporalScale, String> {
    match text(value)?.as_str() {
        "seconds" => Ok(TemporalScale::Seconds),
        "milliseconds" => Ok(TemporalScale::Milliseconds),
        "microseconds" => Ok(TemporalScale::Microseconds),
        "nanoseconds" => Ok(TemporalScale::Nanoseconds),
        _ => Err("unknown recurrence temporal scale".into()),
    }
}

fn weekdays(bits: u64) -> Result<WeekdaySet, String> {
    let all = [
        WeekdaySet::MONDAY,
        WeekdaySet::TUESDAY,
        WeekdaySet::WEDNESDAY,
        WeekdaySet::THURSDAY,
        WeekdaySet::FRIDAY,
        WeekdaySet::SATURDAY,
        WeekdaySet::SUNDAY,
    ];
    if bits == 0 || bits > 0x7f {
        return Err("recurrence weekday set is invalid".into());
    }
    let mut selected = WeekdaySet::MONDAY;
    let mut initialized = false;
    for (index, weekday) in all.into_iter().enumerate() {
        if bits & (1 << index) != 0 {
            selected = if initialized {
                selected.union(weekday)
            } else {
                weekday
            };
            initialized = true;
        }
    }
    Ok(selected)
}

pub(super) fn leaf(kind: &str, text: &str) -> Result<StructuredInfoValue, String> {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(kind)).map_err(structured)?,
        text.as_bytes().to_vec(),
    )
    .map_err(structured)
}

pub(super) fn count(value: u64) -> Result<StructuredInfoValue, String> {
    leaf("value/count@1", &value.to_string())
}

pub(super) fn value_field(name: &str, value: StructuredInfoValue) -> StructuredFieldValue {
    StructuredFieldValue::new(name, value).unwrap()
}

fn scale_name(scale: TemporalScale) -> &'static str {
    match scale {
        TemporalScale::Seconds => "seconds",
        TemporalScale::Milliseconds => "milliseconds",
        TemporalScale::Microseconds => "microseconds",
        TemporalScale::Nanoseconds => "nanoseconds",
    }
}

fn resolution_name(choice: CivilResolutionChoice) -> &'static str {
    match choice {
        CivilResolutionChoice::Unique => "unique",
        CivilResolutionChoice::GapBefore => "gap_before",
        CivilResolutionChoice::GapAfter => "gap_after",
        CivilResolutionChoice::FoldEarlier => "fold_earlier",
        CivilResolutionChoice::FoldLater => "fold_later",
    }
}

fn format_date(date: LocalDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

fn format_time(time: LocalTime) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        time.hour(),
        time.minute(),
        time.second()
    )
}

pub(super) fn structured(error: conduit_core::StructuredInfoRefusal) -> String {
    format!("recurrence structured Info refusal: {error:?}")
}
