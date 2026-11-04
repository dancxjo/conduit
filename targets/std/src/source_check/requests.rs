//! Allocation-free classification against immutable prepared Source scopes.
use super::*;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
use std::ops::Range;
pub(super) const INVALID: usize = 0;
pub(super) const STALE: usize = 1;
pub(super) const GRAMMAR: usize = 2;
pub(super) const CONTEXT: usize = 3;
pub(super) const ENTRY: usize = 4;
pub(super) const NOT_PREPARED: usize = 5;
pub(super) struct Entry {
    pub request: Box<[u8]>,
    pub report: Box<[u8]>,
    snapshot: Range<usize>,
    context: Range<usize>,
}
fn field<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> Option<ValidatedCanonicalStructuredValue<'a>> {
    value.record_field(name).ok().flatten()
}
fn range(whole: &[u8], part: &[u8]) -> Range<usize> {
    let start = part.as_ptr() as usize - whole.as_ptr() as usize;
    start..start + part.len()
}
type PreparedEntry = (Entry, Box<[u8]>, Box<[u8]>);
impl Entry {
    pub(super) fn prepare(request: &[u8], report: &[u8]) -> Result<PreparedEntry, String> {
        let input = validate_canonical_structured_value(request)
            .map_err(|e| format!("request shape: {e:?}"))?;
        let output = validate_canonical_structured_value(report)
            .map_err(|e| format!("reply shape: {e:?}"))?;
        let node = input.value_node();
        let snapshot = range(
            node,
            field(input, "snapshot")
                .ok_or("request snapshot absent")?
                .value_node(),
        );
        let context = range(
            node,
            field(input, "context")
                .ok_or("request context absent")?
                .value_node(),
        );
        Ok((
            Self {
                request: node.into(),
                report: output.value_node().into(),
                snapshot,
                context,
            },
            input.type_bytes().into(),
            output.type_bytes().into(),
        ))
    }
}
pub(super) fn select<'a>(provider: &'a PreparedSourceCheckProvider, input: &[u8]) -> &'a [u8] {
    let refused = |index: usize| provider.refusals[index].as_ref();
    if input.len() > provider.maximum_request {
        return refused(INVALID);
    }
    let Ok(value) = validate_canonical_structured_value(input) else {
        return refused(INVALID);
    };
    if value.type_bytes() != provider.request_schema.as_ref() {
        return refused(INVALID);
    }
    let Some(snapshot) = field(value, "snapshot") else {
        return refused(INVALID);
    };
    let Some(context) = field(value, "context") else {
        return refused(INVALID);
    };
    let mut known = false;
    for entry in &provider.entries {
        if value.value_node() == entry.request.as_ref() {
            return &entry.report;
        }
        if snapshot.value_node() == &entry.request[entry.snapshot.clone()] {
            known = true;
            if context.value_node() == &entry.request[entry.context.clone()] {
                let entry_name = field(value, "entry").and_then(|v| {
                    v.primitive_representation_bytes(conduit_core::TEXT_INFO_ID)
                        .ok()
                });
                if entry_name.is_none_or(|name| {
                    name.is_empty() || name.len() > conduit_plot::MAX_CHECK_LABEL_BYTES
                }) {
                    return refused(ENTRY);
                }
                return refused(NOT_PREPARED);
            }
        }
    }
    if !known {
        return refused(STALE);
    }
    let grammar = field(context, "grammar_edition").and_then(|v| {
        v.primitive_representation_bytes(conduit_core::TEXT_INFO_ID)
            .ok()
    });
    if grammar != Some(b"conduitese-v1".as_slice()) {
        refused(GRAMMAR)
    } else {
        refused(CONTEXT)
    }
}
