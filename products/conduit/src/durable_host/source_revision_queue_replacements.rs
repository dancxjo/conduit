//! Exact local planning-preference CAS; never an offer or execution grant.
use super::*;
use conduit_core::{ConnectionTrack, PortDirection};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LocalForeQueueReplacement {
    entry: String,
    host_id: String,
    front_port_id: String,
    direction: PortDirection,
    track: ConnectionTrack,
    expected_item_capacity: u16,
    expected_byte_capacity: u32,
    selected_item_capacity: u16,
    selected_byte_capacity: u32,
}

fn finite(items: u16, bytes: u32) -> bool {
    (1..=32).contains(&items) && (1..=16384).contains(&bytes)
}

impl LocalForeQueueReplacement {
    fn same_boundary(&self, other: &Self) -> bool {
        self.entry == other.entry
            && self.host_id == other.host_id
            && self.front_port_id == other.front_port_id
            && self.direction == other.direction
            && self.track == other.track
    }
    fn matches(
        &self,
        intent: &super::super::super::local_fore_policy::LocalForeAdapterIntent,
        port: &super::super::super::local_fore_policy::LocalForePortIntent,
    ) -> bool {
        self.entry == intent.entry
            && self.host_id == intent.host_id.as_str()
            && self.front_port_id == port.front_port_id.as_str()
            && self.direction == port.direction
            && self.track == port.track
    }
}

pub(super) fn replace_queues(
    selection: &mut Selection,
    replacements: &[LocalForeQueueReplacement],
) -> Result<(), String> {
    let maximum = conduit_body::MAX_BODY_PLOTS
        .checked_mul(32)
        .ok_or("local Fore replacement count overflow")?;
    if replacements.len() > maximum
        || selection.local_fore_adapters.len() > conduit_body::MAX_BODY_PLOTS
    {
        return Err("local Fore replacements exceed finite workset bounds".into());
    }
    // Refuse malformed original preferences, even if a replacement could repair them.
    for (index, intent) in selection.local_fore_adapters.iter().enumerate() {
        if intent.host_id.as_str().is_empty()
            || intent.configuration_id.is_empty()
            || !selection.entries.contains(&intent.entry)
            || intent.ports.len() > 32
            || selection.local_fore_adapters[..index]
                .iter()
                .any(|old| old.entry == intent.entry)
            || intent.ports.iter().enumerate().any(|(i, port)| {
                port.front_port_id.as_str().is_empty()
                    || !finite(port.item_capacity, port.byte_capacity)
                    || intent.ports[..i].iter().any(|old| {
                        old.front_port_id == port.front_port_id
                            && old.direction == port.direction
                            && old.track == port.track
                    })
            })
        {
            return Err("original local Fore preferences are invalid or ambiguous".into());
        }
    }
    // Validate the entire batch against the unchanged original selection.
    for (index, replacement) in replacements.iter().enumerate() {
        if replacement.entry.is_empty()
            || replacement.entry.len() > conduit_plot::MAX_CHECK_LABEL_BYTES
            || replacement.host_id.is_empty()
            || replacement.host_id.len() > 256
            || replacement.front_port_id.is_empty()
            || !finite(
                replacement.expected_item_capacity,
                replacement.expected_byte_capacity,
            )
            || !finite(
                replacement.selected_item_capacity,
                replacement.selected_byte_capacity,
            )
            || (replacement.expected_item_capacity == replacement.selected_item_capacity
                && replacement.expected_byte_capacity == replacement.selected_byte_capacity)
            || replacements[..index]
                .iter()
                .any(|old| old.same_boundary(replacement))
        {
            return Err("local Fore queue replacement is invalid, duplicate or unchanged".into());
        }
        let mut matching = selection.local_fore_adapters.iter().flat_map(|intent| {
            intent
                .ports
                .iter()
                .filter(move |port| replacement.matches(intent, port))
        });
        let port = matching
            .next()
            .ok_or("local Fore replacement names an unselected boundary")?;
        if matching.next().is_some()
            || port.item_capacity != replacement.expected_item_capacity
            || port.byte_capacity != replacement.expected_byte_capacity
        {
            return Err("local Fore replacement differs from its exact original preference".into());
        }
    }
    for replacement in replacements {
        for intent in &mut selection.local_fore_adapters {
            // Selector identities were checked above and never change in this batch.
            if intent.entry == replacement.entry && intent.host_id.as_str() == replacement.host_id {
                for port in &mut intent.ports {
                    if port.front_port_id.as_str() == replacement.front_port_id
                        && port.direction == replacement.direction
                        && port.track == replacement.track
                    {
                        port.item_capacity = replacement.selected_item_capacity;
                        port.byte_capacity = replacement.selected_byte_capacity;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_revision_queue_replacement_tests.rs"]
mod tests;
