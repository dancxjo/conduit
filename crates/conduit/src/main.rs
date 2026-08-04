use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::thread;
use std::time::Duration;

const PROTOCOL_VERSION: u16 = 1;
const SIGNAL_KIND_ID: &str = "value/signal";
const PULSE_KIND_ID: &str = "flow/pulse";
const SHOW_KIND_ID: &str = "display/show";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Signal {
    sequence: u64,
    level: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostAdvertisement {
    protocol_version: u16,
    host_id: String,
    boot_id: String,
    profile: String,
    capabilities: Vec<CapabilityAdvertisement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapabilityAdvertisement {
    capability_id: String,
    kind_id: String,
    implementation_id: String,
    value_kind: String,
    max_active_instances: usize,
    max_queue_items: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CellKind {
    Pulse,
    Show,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PulseConfig {
    count: u64,
    period_ms: u64,
    initial: bool,
}

impl Default for PulseConfig {
    fn default() -> Self {
        Self {
            count: 16,
            period_ms: 250,
            initial: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Form {
    name: String,
    cells: BTreeMap<String, CellKind>,
    pulse_config: BTreeMap<String, PulseConfig>,
    cords: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlannedCell {
    cell_id: String,
    kind_id: String,
    implementation_id: String,
    capability_id: String,
    host_id: String,
    boot_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlannedCord {
    cord_id: String,
    from: String,
    to: String,
    provider: String,
    queue_capacity: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Plan {
    plan_id: String,
    form_name: String,
    protocol_version: u16,
    cells: Vec<PlannedCell>,
    cords: Vec<PlannedCord>,
    finite_source_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShowReceipt {
    sink_cell_id: String,
    signal: Signal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanReceipts {
    plan_id: String,
    receipts: Vec<ShowReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PlanError {
    NoMatchingPulseCapability,
    NoMatchingShowCapability,
    IncompatibleValueKind {
        capability_id: String,
        got: String,
    },
    QueueRequirementAboveHostLimit {
        required: usize,
        max_supported: usize,
        capability_id: String,
    },
    DuplicateOrInvalidPlacement(String),
    StaleBootId {
        planned_boot_id: String,
        current_boot_id: String,
    },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::NoMatchingPulseCapability => {
                write!(f, "no matching source capability for flow/pulse")
            }
            PlanError::NoMatchingShowCapability => {
                write!(f, "no matching sink capability for display/show<Signal>")
            }
            PlanError::IncompatibleValueKind { capability_id, got } => write!(
                f,
                "capability '{capability_id}' has incompatible value kind '{got}'"
            ),
            PlanError::QueueRequirementAboveHostLimit {
                required,
                max_supported,
                capability_id,
            } => write!(
                f,
                "queue requirement {required} exceeds host limit {max_supported} for capability '{capability_id}'"
            ),
            PlanError::DuplicateOrInvalidPlacement(message) => {
                write!(f, "duplicate or invalid placement: {message}")
            }
            PlanError::StaleBootId {
                planned_boot_id,
                current_boot_id,
            } => write!(
                f,
                "stale host boot id: planned '{planned_boot_id}', current '{current_boot_id}'"
            ),
        }
    }
}

fn std_host_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "std-host-1".to_string(),
        boot_id: "boot-1".to_string(),
        profile: "rust-std".to_string(),
        capabilities: vec![
            CapabilityAdvertisement {
                capability_id: "cap-pulse-1".to_string(),
                kind_id: PULSE_KIND_ID.to_string(),
                implementation_id: "std/pulse-v1".to_string(),
                value_kind: SIGNAL_KIND_ID.to_string(),
                max_active_instances: 16,
                max_queue_items: 4,
            },
            CapabilityAdvertisement {
                capability_id: "cap-show-stdout-1".to_string(),
                kind_id: SHOW_KIND_ID.to_string(),
                implementation_id: "std/stdout-show-signal-v1".to_string(),
                value_kind: SIGNAL_KIND_ID.to_string(),
                max_active_instances: 16,
                max_queue_items: 4,
            },
        ],
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("expected boolean true/false, got '{other}'")),
    }
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|err| format!("expected unsigned integer, got '{}': {err}", value.trim()))
}

fn parse_form(source: &str) -> Result<Form, String> {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("#"))
        .collect();

    if lines.first().copied().unwrap_or("") != "form 0" {
        return Err("expected first non-comment line to be 'form 0'".to_string());
    }
    if lines.len() < 3 {
        return Err("incomplete form".to_string());
    }

    let block_start = lines[1];
    if !block_start.ends_with('{') {
        return Err("expected form block opener like 'name {'".to_string());
    }
    let name = block_start.trim_end_matches('{').trim().to_string();
    if name.is_empty() {
        return Err("form name must not be empty".to_string());
    }
    if lines.last().copied().unwrap_or("") != "}" {
        return Err("expected closing '}' at end of form".to_string());
    }

    let mut cells: BTreeMap<String, CellKind> = BTreeMap::new();
    let mut pulse_config: BTreeMap<String, PulseConfig> = BTreeMap::new();
    let mut cords: Vec<(String, String)> = Vec::new();

    for raw in &lines[2..lines.len() - 1] {
        let line = raw.trim();
        if let Some((left, right)) = line.split_once(':') {
            let cell_id = left.trim().to_string();
            let kind = match right.trim() {
                PULSE_KIND_ID => CellKind::Pulse,
                SHOW_KIND_ID => CellKind::Show,
                other => {
                    return Err(format!(
                        "unsupported kind '{other}'. supported kinds: {PULSE_KIND_ID}, {SHOW_KIND_ID}"
                    ));
                }
            };
            if cells.insert(cell_id.clone(), kind).is_some() {
                return Err(format!("duplicate cell id '{cell_id}'"));
            }
            continue;
        }

        if let Some((left, right)) = line.split_once('=') {
            let (cell_id, key) = left
                .trim()
                .split_once('.')
                .ok_or_else(|| format!("invalid config assignment '{line}'"))?;
            let cell_id = cell_id.trim();
            let key = key.trim();

            let cell_kind = cells
                .get(cell_id)
                .ok_or_else(|| format!("config references unknown cell '{cell_id}'"))?;
            if *cell_kind != CellKind::Pulse {
                return Err(format!(
                    "config key '{key}' is only valid on {PULSE_KIND_ID}, not '{cell_id}'"
                ));
            }

            let cfg = pulse_config.entry(cell_id.to_string()).or_default();
            match key {
                "count" => cfg.count = parse_u64(right)?,
                "period-ms" => cfg.period_ms = parse_u64(right)?,
                "initial" => cfg.initial = parse_bool(right)?,
                other => {
                    return Err(format!(
                        "unsupported pulse config key '{other}' on cell '{cell_id}'"
                    ));
                }
            }
            continue;
        }

        if let Some((from, to)) = line.split_once('>') {
            let from = from.trim().to_string();
            let to = to.trim().to_string();
            cords.push((from, to));
            continue;
        }

        return Err(format!("could not parse form statement '{line}'"));
    }

    if cells.is_empty() {
        return Err("form must declare at least one cell".to_string());
    }
    if cords.is_empty() {
        return Err("form must declare at least one cord".to_string());
    }

    Ok(Form {
        name,
        cells,
        pulse_config,
        cords,
    })
}

fn select_capability<'a>(
    ad: &'a HostAdvertisement,
    kind_id: &str,
) -> Result<&'a CapabilityAdvertisement, PlanError> {
    let cap = ad.capabilities.iter().find(|cap| cap.kind_id == kind_id);
    match cap {
        Some(cap) => {
            if cap.value_kind != SIGNAL_KIND_ID {
                return Err(PlanError::IncompatibleValueKind {
                    capability_id: cap.capability_id.clone(),
                    got: cap.value_kind.clone(),
                });
            }
            Ok(cap)
        }
        None => {
            if kind_id == PULSE_KIND_ID {
                Err(PlanError::NoMatchingPulseCapability)
            } else {
                Err(PlanError::NoMatchingShowCapability)
            }
        }
    }
}

fn plan_local(form: &Form, ad: &HostAdvertisement) -> Result<Plan, PlanError> {
    if ad.protocol_version != PROTOCOL_VERSION {
        return Err(PlanError::DuplicateOrInvalidPlacement(format!(
            "protocol mismatch: expected {PROTOCOL_VERSION}, got {}",
            ad.protocol_version
        )));
    }

    let mut cells: Vec<PlannedCell> = Vec::new();
    let mut planned_pulse: usize = 0;
    let mut planned_show: usize = 0;

    for (cell_id, kind) in &form.cells {
        let (kind_id, cap) = match kind {
            CellKind::Pulse => {
                planned_pulse += 1;
                (PULSE_KIND_ID, select_capability(ad, PULSE_KIND_ID)?)
            }
            CellKind::Show => {
                planned_show += 1;
                (SHOW_KIND_ID, select_capability(ad, SHOW_KIND_ID)?)
            }
        };
        cells.push(PlannedCell {
            cell_id: cell_id.clone(),
            kind_id: kind_id.to_string(),
            implementation_id: cap.implementation_id.clone(),
            capability_id: cap.capability_id.clone(),
            host_id: ad.host_id.clone(),
            boot_id: ad.boot_id.clone(),
        });
    }

    if planned_pulse == 0 {
        return Err(PlanError::DuplicateOrInvalidPlacement(
            "form contains no pulse source".to_string(),
        ));
    }
    if planned_show == 0 {
        return Err(PlanError::DuplicateOrInvalidPlacement(
            "form contains no show sink".to_string(),
        ));
    }

    let pulse_cap = select_capability(ad, PULSE_KIND_ID)?;
    let show_cap = select_capability(ad, SHOW_KIND_ID)?;
    if planned_pulse > pulse_cap.max_active_instances {
        return Err(PlanError::DuplicateOrInvalidPlacement(format!(
            "pulse placements {} exceed capability max {}",
            planned_pulse, pulse_cap.max_active_instances
        )));
    }
    if planned_show > show_cap.max_active_instances {
        return Err(PlanError::DuplicateOrInvalidPlacement(format!(
            "show placements {} exceed capability max {}",
            planned_show, show_cap.max_active_instances
        )));
    }

    let queue_capacity = 4usize;
    if queue_capacity > pulse_cap.max_queue_items {
        return Err(PlanError::QueueRequirementAboveHostLimit {
            required: queue_capacity,
            max_supported: pulse_cap.max_queue_items,
            capability_id: pulse_cap.capability_id.clone(),
        });
    }
    if queue_capacity > show_cap.max_queue_items {
        return Err(PlanError::QueueRequirementAboveHostLimit {
            required: queue_capacity,
            max_supported: show_cap.max_queue_items,
            capability_id: show_cap.capability_id.clone(),
        });
    }

    let mut cords: Vec<PlannedCord> = Vec::new();
    for (index, (from, to)) in form.cords.iter().enumerate() {
        let from_kind = form.cells.get(from).ok_or_else(|| {
            PlanError::DuplicateOrInvalidPlacement(format!("cord source '{from}' does not exist"))
        })?;
        let to_kind = form.cells.get(to).ok_or_else(|| {
            PlanError::DuplicateOrInvalidPlacement(format!("cord sink '{to}' does not exist"))
        })?;
        if *from_kind != CellKind::Pulse {
            return Err(PlanError::DuplicateOrInvalidPlacement(format!(
                "cord source '{from}' is not {PULSE_KIND_ID}"
            )));
        }
        if *to_kind != CellKind::Show {
            return Err(PlanError::DuplicateOrInvalidPlacement(format!(
                "cord sink '{to}' is not {SHOW_KIND_ID}"
            )));
        }
        cords.push(PlannedCord {
            cord_id: format!("cord-{}", index + 1),
            from: from.clone(),
            to: to.clone(),
            provider: "local".to_string(),
            queue_capacity,
        });
    }

    let source_count = form
        .cells
        .iter()
        .find_map(|(cell_id, kind)| {
            if *kind == CellKind::Pulse {
                Some(
                    form.pulse_config
                        .get(cell_id)
                        .cloned()
                        .unwrap_or_default()
                        .count,
                )
            } else {
                None
            }
        })
        .ok_or_else(|| {
            PlanError::DuplicateOrInvalidPlacement("form has no pulse source".to_string())
        })?;

    Ok(Plan {
        plan_id: format!("plan-{}-{}", form.name, ad.boot_id),
        form_name: form.name.clone(),
        protocol_version: PROTOCOL_VERSION,
        cells,
        cords,
        finite_source_count: source_count,
    })
}

fn activate_local(
    plan: &Plan,
    form: &Form,
    ad: &HostAdvertisement,
) -> Result<PlanReceipts, PlanError> {
    let planned_boot_id = plan
        .cells
        .first()
        .map(|cell| cell.boot_id.clone())
        .unwrap_or_default();
    if ad.boot_id != planned_boot_id {
        return Err(PlanError::StaleBootId {
            planned_boot_id,
            current_boot_id: ad.boot_id.clone(),
        });
    }

    let pulse_cell = form
        .cells
        .iter()
        .find_map(|(cell_id, kind)| {
            if *kind == CellKind::Pulse {
                Some(cell_id.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            PlanError::DuplicateOrInvalidPlacement("missing pulse cell in form".to_string())
        })?;
    let pulse_cfg = form
        .pulse_config
        .get(&pulse_cell)
        .cloned()
        .unwrap_or_default();

    let mut receipts = Vec::new();
    for sequence in 0..pulse_cfg.count {
        let level = if sequence % 2 == 0 {
            pulse_cfg.initial
        } else {
            !pulse_cfg.initial
        };
        let signal = Signal { sequence, level };
        for cord in &plan.cords {
            let state = if signal.level { "on" } else { "off" };
            if plan.cords.len() == 1 {
                println!("signal {} {}", signal.sequence, state);
            } else {
                println!("{} signal {} {}", cord.to, signal.sequence, state);
            }
            receipts.push(ShowReceipt {
                sink_cell_id: cord.to.clone(),
                signal: signal.clone(),
            });
        }

        if pulse_cfg.period_ms > 0 {
            thread::sleep(Duration::from_millis(pulse_cfg.period_ms));
        }
    }

    println!("plan {} complete", plan.plan_id);

    Ok(PlanReceipts {
        plan_id: plan.plan_id.clone(),
        receipts,
    })
}

fn run(path: &str) -> Result<(), String> {
    let source =
        fs::read_to_string(path).map_err(|err| format!("failed to read '{path}': {err}"))?;
    let form = parse_form(&source)?;
    let host_ad = std_host_advertisement();
    let plan = plan_local(&form, &host_ad).map_err(|err| err.to_string())?;
    let receipts = activate_local(&plan, &form, &host_ad).map_err(|err| err.to_string())?;

    let first = receipts.receipts.first().cloned();
    let last = receipts.receipts.last().cloned();
    if let (Some(first), Some(last)) = (first, last) {
        println!(
            "receipts {} first=({}, {}) last=({}, {})",
            receipts.receipts.len(),
            first.signal.sequence,
            first.signal.level,
            last.signal.sequence,
            last.signal.level
        );
    } else {
        println!("receipts 0");
    }

    Ok(())
}

fn main() {
    let mut args = env::args();
    let _program = args.next();
    let path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("usage: conduit <form-file>");
            std::process::exit(2);
        }
    };

    if let Err(err) = run(&path) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_form() -> &'static str {
        "form 0\n\nsignal-demo {\n    pulse: flow/pulse\n    show: display/show\n\n    pulse.count = 3\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n"
    }

    #[test]
    fn parses_signal_demo_form() {
        let form = parse_form(fixture_form()).expect("form should parse");
        assert_eq!(form.name, "signal-demo");
        assert_eq!(form.cells.len(), 2);
        assert_eq!(form.cords, vec![("pulse".to_string(), "show".to_string())]);
        assert_eq!(form.pulse_config.get("pulse").unwrap().count, 3);
    }

    #[test]
    fn planning_fails_without_show_capability() {
        let form = parse_form(fixture_form()).expect("form should parse");
        let mut ad = std_host_advertisement();
        ad.capabilities.retain(|cap| cap.kind_id != SHOW_KIND_ID);
        let err = plan_local(&form, &ad).expect_err("planning must fail");
        assert_eq!(err, PlanError::NoMatchingShowCapability);
    }

    #[test]
    fn planning_fails_with_queue_above_limit() {
        let form = parse_form(fixture_form()).expect("form should parse");
        let mut ad = std_host_advertisement();
        for cap in &mut ad.capabilities {
            cap.max_queue_items = 2;
        }
        let err = plan_local(&form, &ad).expect_err("planning must fail");
        assert!(matches!(
            err,
            PlanError::QueueRequirementAboveHostLimit { .. }
        ));
    }

    #[test]
    fn activation_fails_on_stale_boot_id() {
        let form = parse_form(fixture_form()).expect("form should parse");
        let ad = std_host_advertisement();
        let plan = plan_local(&form, &ad).expect("planning should pass");

        let mut restarted = ad.clone();
        restarted.boot_id = "boot-2".to_string();
        let err = activate_local(&plan, &form, &restarted).expect_err("activation must fail");
        assert!(matches!(err, PlanError::StaleBootId { .. }));
    }

    #[test]
    fn local_plan_has_exact_finite_bound() {
        let form = parse_form(fixture_form()).expect("form should parse");
        let ad = std_host_advertisement();
        let plan = plan_local(&form, &ad).expect("planning should pass");
        assert_eq!(plan.finite_source_count, 3);
        assert_eq!(plan.cords.len(), 1);
        assert_eq!(plan.cords[0].provider, "local");
        assert_eq!(plan.cords[0].queue_capacity, 4);
    }
}
