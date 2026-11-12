//! Lossless fact-first display over the exact composed snapshot, not a lookup.
//! The native pending projection stays unchanged. Each displayed value has an
//! explicit reference and encoding in its original evidence metadata. Columns
//! compress keys only; presence masks distinguish missing fields from null.
use super::{writer::*, R};
use crate::episode_retrieval_projection::raw_items::{array_values, object_fields, text_field};
use alloc::vec::Vec;
mod tuples;
use tuples::{Dictionary, CLASS_FIELDS, TIME_FIELDS};
const SCHEMA: &[u8] = super::EPISODE_RETRIEVAL_FACT_DATA_SCHEMA.as_bytes();

fn field<'a>(input: &'a [u8], key: &str) -> Result<&'a [u8], R> {
    let value = text_field(input, key).map_err(|_| R::MalformedExperience)?;
    Ok(value.as_bytes())
}
fn required<'a>(input: &'a [u8], key: &str) -> Result<&'a [u8], R> {
    let value = field(input, key)?;
    if value.is_empty() {
        Err(R::MalformedExperience)
    } else {
        Ok(value)
    }
}
fn array(input: &[u8]) -> Result<impl Iterator<Item = Result<&[u8], R>>, R> {
    Ok(array_values(input)
        .map_err(|_| R::MalformedExperience)?
        .map(|v| v.map_err(|_| R::MalformedExperience)))
}
type RawField<'a> = (&'a [u8], &'a [u8]);
fn fields(input: &[u8]) -> Result<impl Iterator<Item = Result<RawField<'_>, R>>, R> {
    Ok(object_fields(input)
        .map_err(|_| R::MalformedExperience)?
        .map(|v| v.map_err(|_| R::MalformedExperience)))
}

// JSON object content is presented directly, retaining all original lexemes.
// Other UTF-8 and binary values stay explicitly typed. Decode scratch is fixed
// by the already admitted 4 KiB document; there is no play-time allocation.
fn content<T>(
    item: &[u8],
    question: &[u8],
    use_value: impl FnOnce(&[u8], &[u8]) -> Result<T, R>,
) -> Result<T, R> {
    let utf8 = field(item, "utf8_content")?;
    let reference = field(item, "json_string_ref")?;
    let binary = field(item, "content_bytes")?;
    let mut content_fields = 0;
    for pair in fields(item)? {
        let (key, _) = pair?;
        if [
            br#""utf8_content""#.as_slice(),
            br#""json_string_ref""#.as_slice(),
            br#""content_bytes""#.as_slice(),
        ]
        .contains(&key)
        {
            content_fields += 1;
        }
    }
    if content_fields != 1 {
        return Err(R::MalformedExperience);
    }
    if !reference.is_empty() {
        if reference != br#""observation.content""# {
            return Err(R::MalformedExperience);
        }
        return use_value(question, b"json_ref");
    }
    if !binary.is_empty() {
        return use_value(binary, b"binary");
    }
    let mut scratch = [0; 4096];
    if let Ok((decoded, _)) =
        serde_json_core::from_slice_escaped::<crate::FixedEvidenceText<4096>>(utf8, &mut scratch)
    {
        if decoded.as_str().trim() == decoded.as_str()
            && conduit_web::validate_json_text(decoded.as_str().as_bytes()).is_ok()
        {
            return use_value(decoded.as_str().as_bytes(), b"json");
        }
    }
    use_value(utf8, b"utf8")
}
fn original_content(item: &[u8]) -> Result<&[u8], R> {
    for key in ["utf8_content", "json_string_ref", "content_bytes"] {
        let value = field(item, key)?;
        if !value.is_empty() {
            return Ok(value);
        }
    }
    Err(R::MalformedExperience)
}
fn reference(out: &mut Vec<u8>, episode: Option<usize>, item: usize) -> Result<(), R> {
    put(out, b"\"")?;
    if let Some(episode) = episode {
        put(out, b"/remembered_facts/")?;
        integer(out, episode as u64)?;
        put(out, b"/reported/")?;
    } else {
        put(out, b"/current_facts/")?;
    }
    integer(out, item as u64)?;
    put(out, b"\"")
}

pub(super) fn render(data: &[u8], instruction: &[u8], out: &mut Vec<u8>) -> Result<(), R> {
    let current = required(data, "current_experience")?;
    let current_items = required(current, "evidence")?;
    let recalled = required(data, "recalled")?;
    let question = required(required(data, "observation")?, "content")?;
    put(
        out,
        br#"{"schema":"conduit.ai/generation-context@2","instruction":"#,
    )?;
    put(out, instruction)?;
    put(out, br#","question":"#)?;
    put(out, question)?;
    put(out, br#","data":{"current_facts":["#)?;
    for (i, item) in array(current_items)?.enumerate() {
        if i != 0 {
            put(out, b",")?;
        }
        content(item?, question, |value, _| put(out, value))?;
    }
    put(out, br#"],"remembered_facts":["#)?;
    for (i, episode) in array(recalled)?.enumerate() {
        let episode = episode?;
        if i != 0 {
            put(out, b",")?;
        }
        put(out, br#"{"reported":["#)?;
        let historical = required(episode, "historical_evidence")?;
        for (j, item) in array(required(historical, "evidence")?)?.enumerate() {
            if j != 0 {
                put(out, b",")?;
            }
            content(item?, question, |value, _| put(out, value))?;
        }
        put(out, br#"],"actual_outcome":"#)?;
        put(out, required(episode, "actual_outcome")?)?;
        put(out, b"}")?;
    }
    put(out, br#"],"provenance":{"#)?;
    for (i, pair) in fields(data)?.enumerate() {
        let (key, value) = pair?;
        if i != 0 {
            put(out, b",")?;
        }
        put(out, key)?;
        put(out, b":")?;
        match key {
            br#""schema""# => string(out, SCHEMA)?,
            br#""current_experience""# => current_metadata(out, value, question)?,
            br#""recalled""# => recalled_metadata(out, value, question)?,
            br#""observation""# => {
                put(out, b"{")?;
                let mut written = false;
                for pair in fields(value)? {
                    let (key, value) = pair?;
                    if value.as_ptr() == question.as_ptr() {
                        continue;
                    }
                    if written {
                        put(out, b",")?;
                    }
                    put(out, key)?;
                    put(out, b":")?;
                    put(out, value)?;
                    written = true;
                }
                put(out, b"}")?;
            }
            _ => put(out, value)?,
        }
    }
    put(out, b"}}}")
}

fn current_metadata(out: &mut Vec<u8>, document: &[u8], question: &[u8]) -> Result<(), R> {
    let items = required(document, "evidence")?;
    let classes = Dictionary::prepare(
        document,
        "classification_fields",
        &[br#""classifications""#],
        CLASS_FIELDS,
    )?;
    let times = Dictionary::prepare(
        document,
        "time_fields",
        &[br#""observed_at""#, br#""recorded_at""#],
        TIME_FIELDS,
    )?;
    if !field(document, "evidence_fields")?.is_empty()
        || !field(document, "evidence_presence")?.is_empty()
        || !field(document, "evidence_defaults")?.is_empty()
        || !field(document, "classification_defaults")?.is_empty()
        || !field(document, "time_defaults")?.is_empty()
    {
        return Err(R::MalformedExperience);
    }
    let mut columns: [Option<&[u8]>; 16] = [None; 16];
    let mut count = 0;
    let mut sparse = false;
    let mut first_mask = None;
    let mut item_count = 0;
    for item in array(items)? {
        let item = item?;
        item_count += 1;
        let old_content = original_content(item)?;
        let mut mask = 0u16;
        for pair in fields(item)? {
            let (key, value) = pair?;
            if value.as_ptr() == old_content.as_ptr() {
                continue;
            }
            if [
                br#""content_ref""#.as_slice(),
                br#""content_encoding""#.as_slice(),
            ]
            .contains(&key)
            {
                return Err(R::MalformedExperience);
            }
            let position = if let Some(i) = columns[..count].iter().position(|v| *v == Some(key)) {
                i
            } else {
                if count == columns.len() {
                    return Err(R::MalformedExperience);
                }
                columns[count] = Some(key);
                count += 1;
                count - 1
            };
            if mask & (1 << position) != 0 {
                return Err(R::MalformedExperience);
            }
            mask |= 1 << position;
        }
        if first_mask.is_some_and(|first| first != mask) {
            sparse = true;
        }
        first_mask.get_or_insert(mask);
    }
    // A shared column is emitted once only when every item explicitly carries
    // the identical bytes. Missing values never acquire a default by inference.
    let mut defaults: [Option<&[u8]>; 16] = [None; 16];
    if item_count > 1 {
        for (i, column) in columns[..count].iter().enumerate() {
            let mut same = true;
            for item in array(items)? {
                let mut value = None;
                for pair in fields(item?)? {
                    let (key, candidate) = pair?;
                    if Some(key) == *column {
                        value = Some(candidate);
                        break;
                    }
                }
                let Some(value) = value else {
                    same = false;
                    break;
                };
                if defaults[i].is_some_and(|first| first != value) {
                    same = false;
                    break;
                }
                defaults[i] = Some(value);
            }
            if !same {
                defaults[i] = None;
            }
        }
    }
    let active_count = defaults[..count]
        .iter()
        .filter(|value| value.is_none())
        .count();
    put(out, b"{")?;
    classes.defaults(out, br#""classification_defaults""#)?;
    times.defaults(out, br#""time_defaults""#)?;
    if defaults[..count].iter().any(Option::is_some) {
        put(out, br#""evidence_defaults":{"#)?;
        let mut written = false;
        for (column, default) in columns[..count].iter().zip(&defaults[..count]) {
            if let Some(value) = default {
                if written {
                    put(out, b",")?;
                }
                put(out, column.ok_or(R::MalformedExperience)?)?;
                put(out, b":")?;
                tuple_value(
                    out,
                    column.ok_or(R::MalformedExperience)?,
                    value,
                    &classes,
                    &times,
                )?;
                written = true;
            }
        }
        put(out, b"},")?;
    }
    put(out, br#""evidence_fields":["#)?;
    let mut written = false;
    for (i, key) in columns[..count].iter().enumerate() {
        if defaults[i].is_some() {
            continue;
        }
        if written {
            put(out, b",")?;
        }
        put(out, key.ok_or(R::MalformedExperience)?)?;
        written = true;
    }
    if written {
        put(out, b",")?;
    }
    put(out, br#""content_ref","content_encoding"],"#)?;
    if sparse {
        put(out, br#""evidence_presence":["#)?;
        for (i, item) in array(items)?.enumerate() {
            if i != 0 {
                put(out, b",")?;
            }
            let item = item?;
            let mut mask = 0u16;
            for pair in fields(item)? {
                let (key, _) = pair?;
                let mut position = 0;
                for (j, column) in columns[..count].iter().enumerate() {
                    if defaults[j].is_some() {
                        continue;
                    }
                    if *column == Some(key) {
                        mask |= 1 << position;
                    }
                    position += 1;
                }
            }
            // Reference and encoding columns are always present.
            integer(
                out,
                u64::from(mask) | (1u64 << active_count) | (1u64 << (active_count + 1)),
            )?;
        }
        put(out, b"],")?;
    }
    for (i, pair) in fields(document)?.enumerate() {
        let (key, value) = pair?;
        if i != 0 {
            put(out, b",")?;
        }
        put(out, key)?;
        put(out, b":")?;
        if value.as_ptr() != items.as_ptr() {
            match key {
                br#""classification_fields""# => classes.declaration(out, value)?,
                br#""time_fields""# => times.declaration(out, value)?,
                _ => put(out, value)?,
            }
            continue;
        }
        put(out, b"[")?;
        for (j, item) in array(items)?.enumerate() {
            if j != 0 {
                put(out, b",")?;
            }
            let item = item?;
            put(out, b"[")?;
            let mut written = false;
            for (k, column) in columns[..count].iter().enumerate() {
                if defaults[k].is_some() {
                    continue;
                }
                if written {
                    put(out, b",")?;
                }
                written = true;
                let mut found = None;
                for pair in fields(item)? {
                    let (key, value) = pair?;
                    if Some(key) == *column {
                        found = Some(value);
                        break;
                    }
                }
                tuple_value(
                    out,
                    column.ok_or(R::MalformedExperience)?,
                    found.unwrap_or(b"null"),
                    &classes,
                    &times,
                )?;
            }
            if written {
                put(out, b",")?;
            }
            reference(out, None, j)?;
            put(out, b",")?;
            content(item, question, |_, encoding| string(out, encoding))?;
            put(out, b"]")?;
        }
        put(out, b"]")?;
    }
    put(out, b"}")
}

fn tuple_value(
    out: &mut Vec<u8>,
    key: &[u8],
    value: &[u8],
    classes: &Dictionary<'_>,
    times: &Dictionary<'_>,
) -> Result<(), R> {
    match key {
        br#""classifications""# => classes.value(out, value),
        br#""observed_at""# | br#""recorded_at""# => times.value(out, value),
        _ => put(out, value),
    }
}

fn recalled_metadata(out: &mut Vec<u8>, recalled: &[u8], question: &[u8]) -> Result<(), R> {
    put(out, b"[")?;
    for (i, episode) in array(recalled)?.enumerate() {
        let episode = episode?;
        if i != 0 {
            put(out, b",")?;
        }
        let historical = required(episode, "historical_evidence")?;
        put(out, b"{")?;
        for (j, pair) in fields(episode)?.enumerate() {
            let (key, value) = pair?;
            if j != 0 {
                put(out, b",")?;
            }
            put(out, key)?;
            put(out, b":")?;
            if value.as_ptr() == historical.as_ptr() {
                historical_metadata(out, value, question, i)?;
            } else {
                put(out, value)?;
            }
        }
        put(out, b"}")?;
    }
    put(out, b"]")
}
fn historical_metadata(
    out: &mut Vec<u8>,
    document: &[u8],
    question: &[u8],
    episode: usize,
) -> Result<(), R> {
    let items = required(document, "evidence")?;
    put(out, b"{")?;
    for (i, pair) in fields(document)?.enumerate() {
        let (key, value) = pair?;
        if i != 0 {
            put(out, b",")?;
        }
        put(out, key)?;
        put(out, b":")?;
        if value.as_ptr() != items.as_ptr() {
            put(out, value)?;
            continue;
        }
        put(out, b"[")?;
        for (j, item) in array(items)?.enumerate() {
            let item = item?;
            if j != 0 {
                put(out, b",")?;
            }
            let old_content = original_content(item)?;
            put(out, b"{")?;
            for pair in fields(item)? {
                let (key, value) = pair?;
                if value.as_ptr() == old_content.as_ptr() {
                    continue;
                }
                put(out, key)?;
                put(out, b":")?;
                put(out, value)?;
                put(out, b",")?;
            }
            put(out, br#""content_ref":"#)?;
            reference(out, Some(episode), j)?;
            put(out, br#","content_encoding":"#)?;
            content(item, question, |_, encoding| string(out, encoding))?;
            put(out, b"}")?;
        }
        put(out, b"]")?;
    }
    put(out, b"}")
}

#[cfg(test)]
mod tests;
