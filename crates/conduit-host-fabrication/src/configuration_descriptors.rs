use crate::{FabricationCatalog, HostBounds};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTargetDescriptor {
    pub label: &'static str,
    pub family: &'static str,
    pub architecture: &'static str,
    pub machine: &'static str,
    pub board: Option<&'static str>,
    pub os: Option<&'static str>,
    pub host_core: &'static str,
    pub base_implementations: Vec<(&'static str, Vec<&'static str>)>,
    pub presenter: Option<(&'static str, &'static str, bool)>,
    pub host_operations: Vec<&'static str>,
    pub maxima: HostBounds,
}

macro_rules! descriptor {
    ($label:expr, $family:expr, $architecture:expr, $machine:expr, $board:expr,
     $os:expr, $host_core:expr, $memory:expr, $bases:expr, $presenter:expr, $operations:expr $(,)?) => {
        HostTargetDescriptor {
            label: $label,
            family: $family,
            architecture: $architecture,
            machine: $machine,
            board: $board,
            os: $os,
            host_core: $host_core,
            base_implementations: $bases,
            presenter: $presenter,
            host_operations: $operations,
            maxima: HostBounds {
                static_memory_bytes: $memory,
                heap_arena_bytes: $memory,
                queue_items: 65_536,
                buffered_bytes: $memory,
                active_instances: 4_096,
                operation_slots: 4_096,
                timer_slots: 4_096,
                line_sessions: 1_024,
                evidence_items: 65_536,
            },
        }
    };
}

pub fn target_descriptors() -> Vec<HostTargetDescriptor> {
    vec![
        descriptor!(
            "Hosted Linux workstation",
            "std",
            "x86_64",
            "workstation",
            None,
            Some("linux"),
            "host-core/std@1",
            2 * 1024 * 1024 * 1024,
            vec![
                ("clock/monotonic", vec!["hosted/monotonic-clock@1"]),
                ("serial/text", vec!["hosted/serial@1"]),
                ("storage/protected-file", vec!["hosted/protected-file@1"]),
                ("timer/monotonic", vec!["hosted/monotonic-clock@1"]),
            ],
            None,
            Vec::new(),
        ),
        descriptor!(
            "Pico W",
            "conduitos",
            "thumbv6m",
            "pico-w",
            Some("pico-w"),
            None,
            "host-core/conduitos@1",
            256 * 1024,
            vec![("serial/text", vec!["pico/usb-cdc@1"])],
            None,
            Vec::new(),
        ),
        descriptor!(
            "Browser page",
            "browser",
            "wasm32",
            "page",
            None,
            None,
            "host-core/std@1",
            64 * 1024 * 1024,
            vec![("browser/dom", vec!["browser/dom@1"])],
            None,
            Vec::new(),
        ),
        descriptor!(
            "ConduitOS aarch64 virt",
            "conduitos",
            "aarch64",
            "virt",
            None,
            None,
            "host-core/conduitos@1",
            512 * 1024 * 1024,
            vec![("serial/text", vec!["conduitos/pl011@1"])],
            Some(("presenter/main", "presenter/linear-serial@1", false)),
            vec!["conduit.host/present@1"],
        ),
    ]
}

pub fn compatible_base_implementations(
    descriptor: &HostTargetDescriptor,
    catalog: &FabricationCatalog,
) -> Vec<(String, Vec<String>)> {
    let target = format!(
        "{}/{}/{}",
        descriptor.family, descriptor.architecture, descriptor.machine
    );
    let mut choices = descriptor
        .base_implementations
        .iter()
        .filter_map(|(kind, allowed)| {
            if !catalog
                .base_targets
                .get(*kind)
                .is_some_and(|targets| target_matches(targets, &target))
            {
                return None;
            }
            let mut implementations = allowed
                .iter()
                .filter_map(|implementation| {
                    catalog
                        .driver_targets
                        .get(*implementation)
                        .is_some_and(|targets| target_matches(targets, &target))
                        .then_some((*implementation).to_owned())
                })
                .collect::<Vec<_>>();
            implementations.sort();
            (!implementations.is_empty()).then_some(((*kind).to_owned(), implementations))
        })
        .collect::<Vec<_>>();
    choices.sort();
    choices
}

fn target_matches(patterns: &[String], target: &str) -> bool {
    let actual = target.split('/').collect::<Vec<_>>();
    patterns.iter().any(|pattern| {
        pattern
            .split('/')
            .zip(&actual)
            .all(|(expected, found)| expected == "*" || expected == *found)
    })
}
