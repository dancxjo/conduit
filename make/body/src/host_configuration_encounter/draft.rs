use super::*;
use conduit_host_make::{BaseInitialization, ConfigurationBase, ConfigurationTarget, HostBounds};
use std::collections::BTreeMap;

pub(super) const LIMITS: [&str; 9] = [
    "static_memory_bytes",
    "heap_arena_bytes",
    "queue_items",
    "buffered_bytes",
    "active_instances",
    "operation_slots",
    "timer_slots",
    "line_sessions",
    "evidence_items",
];
pub(super) fn diagnostics(errors: Vec<conduit_host_make::ConfigurationDiagnostic>) -> String {
    let text = errors
        .iter()
        .take(3)
        .map(|e| format!("{e:?}"))
        .collect::<Vec<_>>()
        .join("; ");
    bounded_text(&text, 768)
}
pub(super) fn bounds(c: &HostConfiguration) -> Result<(), String> {
    if c.bases.len() > MAX_HOST_ENCOUNTER_BASES || c.name.len() > 64 || c.resources.len() > 16 {
        return Err("Host draft inventory exceeds encounter bound".into());
    }
    for base in &c.bases {
        if base.implementations.len() > 8 {
            return Err("Host Base alternatives exceed encounter bound".into());
        }
        if let Some(init) = &base.initialization {
            if init.values.len() > 16
                || init
                    .values
                    .values()
                    .any(|v| v.as_str().is_some_and(|s| s.len() > 1024))
            {
                return Err("Host initialization exceeds encounter field bound".into());
            }
        }
    }
    if serde_json::to_vec(c)
        .map_err(|_| "Host draft encoding refused")?
        .len()
        > MAX_HOST_ENCOUNTER_SOURCE_BYTES
    {
        return Err("Host draft exceeds encounter byte bound".into());
    }
    Ok(())
}
pub(super) fn target_key(
    c: &HostConfiguration,
    e: &HostConfigurationEncounter<'_>,
) -> Result<String, String> {
    e.packages
        .target_descriptors()
        .into_iter()
        .find(|t| {
            t.architecture == c.target.architecture
                && t.machine == c.target.machine
                && t.os == c.target.os
                && t.board == c.target.board
        })
        .map(|t| t.key())
        .ok_or_else(|| "Host target absent from catalogue".into())
}
pub(super) fn offers(
    e: &HostConfigurationEncounter<'_>,
) -> Result<Vec<conduit_host_make::ResolvedImplementationOffer>, String> {
    let c = e.configuration.as_ref().ok_or("Select a target first")?;
    let offers = e.packages.offers_for_target(&target_key(c, e)?);
    if offers.len() > MAX_CHOICES {
        return Err("Host Base catalogue exceeds encounter bound".into());
    }
    Ok(offers)
}
pub(super) fn apply(
    e: &HostConfigurationEncounter<'_>,
    c: &mut Option<HostConfiguration>,
    action: &str,
    value: &str,
) -> Result<Page, String> {
    let page = e.page;
    if action == "host-config/home" {
        return Ok(Page::Main);
    }
    if action == "host-config/targets" {
        return Ok(Page::Targets(0));
    }
    if action == "host-config/bases" {
        return Ok(Page::Bases(0));
    }
    if action == "host-config/source" {
        e.canonical_source()?;
        return Ok(Page::Source(0));
    }
    if action == "host-config/check" {
        return Ok(page);
    }
    if action == "host-config/next" || action == "host-config/previous" {
        let next = |p: usize| {
            if action.ends_with("next") {
                p + 1
            } else {
                p.saturating_sub(1)
            }
        };
        return match page {
            Page::Targets(p) => Ok(Page::Targets(next(p))),
            Page::Bases(p) => Ok(Page::Bases(next(p))),
            Page::Source(p) => Ok(Page::Source(next(p))),
            _ => Err("No page navigation".into()),
        };
    }
    if let Some(index) = action.strip_prefix("host-config/target/") {
        let index: usize = index.parse().map_err(|_| "Invalid target choice")?;
        let targets = e.packages.target_descriptors();
        let t = targets.get(index).ok_or("Unknown target")?;
        *c = Some(HostConfiguration {
            schema: 1,
            name: "".into(),
            target: ConfigurationTarget {
                architecture: t.architecture.clone(),
                machine: t.machine.clone(),
                board: t.board.clone(),
                os: t.os.clone(),
                make_descriptor: None,
            },
            bases: vec![],
            resources: vec![],
            limits: HostBounds {
                static_memory_bytes: 0,
                heap_arena_bytes: 0,
                queue_items: 0,
                buffered_bytes: 0,
                active_instances: 0,
                operation_slots: 0,
                timer_slots: 0,
                line_sessions: 0,
                evidence_items: 0,
            },
        });
        return Ok(Page::Main);
    }
    let config = c.as_mut().ok_or("Select a target first")?;
    if action == "host-config/descriptor" {
        let key = target_key(config, e)?;
        let target = e.packages.target_descriptor(&key).ok_or("Unknown target")?;
        if !target
            .make_descriptors
            .iter()
            .any(|descriptor| descriptor == value)
        {
            return Err("Descriptor is not offered by the selected target".into());
        }
        config.target.make_descriptor = Some(value.into());
        return Ok(page);
    }
    if action == "host-config/name" {
        config.name = value.into();
        return Ok(page);
    }
    if let Some(key) = action.strip_prefix("host-config/limit/") {
        set_limit(
            &mut config.limits,
            key,
            value
                .parse()
                .map_err(|_| "Limit requires an unsigned integer")?,
        )?;
        return Ok(page);
    }
    if let Some(index) = action.strip_prefix("host-config/add/") {
        let index: usize = index.parse().map_err(|_| "Invalid Base choice")?;
        let offers = offers(e)?;
        let offer = &offers.get(index).ok_or("Unknown Base choice")?.offer;
        if config.bases.iter().any(|b| b.kind == offer.base_kind) {
            return Err("Base already selected; remove explicitly before replacing".into());
        }
        config.bases.push(ConfigurationBase {
            kind: offer.base_kind.clone(),
            implementation: Some(offer.implementation_id.clone()),
            implementations: vec![],
            initialization: None,
        });
        return Ok(Page::Main);
    }
    if let Some(index) = action.strip_prefix("host-config/remove/") {
        let index: usize = index.parse().map_err(|_| "Invalid selected Base")?;
        if index >= config.bases.len() {
            return Err("Unknown selected Base".into());
        }
        config.bases.remove(index);
        return Ok(Page::Main);
    }
    if let Some(index) = action.strip_prefix("host-config/initialize/") {
        let index: usize = index.parse().map_err(|_| "Invalid selected Base")?;
        if index >= config.bases.len() {
            return Err("Unknown selected Base".into());
        }
        return Ok(Page::Initialization(index));
    }
    if action == "host-config/clear-initialization" {
        if let Page::Initialization(i) = page {
            config.bases[i].initialization = None;
            return Ok(page);
        }
    }
    if let Some(key) = action.strip_prefix("host-config/field/") {
        let Page::Initialization(i) = page else {
            return Err("No selected initialization".into());
        };
        let base = &mut config.bases[i];
        let implementation = base
            .implementation
            .as_ref()
            .ok_or("Select an exact implementation")?;
        let contract = e
            .catalog
            .initialization_contracts
            .get(implementation)
            .ok_or("Implementation has no initialization contract")?;
        let field = contract
            .fields
            .iter()
            .find(|f| f.key == key)
            .ok_or("Unknown initialization field")?;
        use conduit_core::{ConfigurationValue as V, KindConfigurationRule as R};
        let typed = match &field.rule {
            R::U64Range { .. } | R::DurationMillis { .. } => V::U64(
                value
                    .parse()
                    .map_err(|_| "Initialization requires unsigned integer")?,
            ),
            _ => V::Text(value.into()),
        };
        conduit_plot::validate_configuration_value(field, &typed)
            .map_err(|_| "Initialization value violates selected contract")?;
        let json = match typed {
            V::U64(v) => serde_json::Value::from(v),
            V::Text(v) => serde_json::Value::from(v),
            _ => return Err("Unsupported initialization field type".into()),
        };
        base.initialization
            .get_or_insert_with(|| BaseInitialization {
                values: BTreeMap::new(),
            })
            .values
            .insert(key.into(), json);
        return Ok(page);
    }
    Err("Unknown Host authoring action".into())
}
pub(super) fn limit(b: &HostBounds, key: &str) -> u64 {
    match key {
        "static_memory_bytes" => b.static_memory_bytes,
        "heap_arena_bytes" => b.heap_arena_bytes,
        "queue_items" => b.queue_items.into(),
        "buffered_bytes" => b.buffered_bytes,
        "active_instances" => b.active_instances.into(),
        "operation_slots" => b.operation_slots.into(),
        "timer_slots" => b.timer_slots.into(),
        "line_sessions" => b.line_sessions.into(),
        _ => b.evidence_items.into(),
    }
}
fn set_limit(b: &mut HostBounds, key: &str, v: u64) -> Result<(), String> {
    match key {
        "static_memory_bytes" => b.static_memory_bytes = v,
        "heap_arena_bytes" => b.heap_arena_bytes = v,
        "buffered_bytes" => b.buffered_bytes = v,
        _ => {
            let v = v.try_into().map_err(|_| "Limit exceeds u32")?;
            match key {
                "queue_items" => b.queue_items = v,
                "active_instances" => b.active_instances = v,
                "operation_slots" => b.operation_slots = v,
                "timer_slots" => b.timer_slots = v,
                "line_sessions" => b.line_sessions = v,
                "evidence_items" => b.evidence_items = v,
                _ => return Err("Unknown Host limit".into()),
            }
        }
    }
    Ok(())
}

pub(super) fn bounded_text(text: &str, maximum: usize) -> String {
    let mut end = text.len().min(maximum);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].into()
}
