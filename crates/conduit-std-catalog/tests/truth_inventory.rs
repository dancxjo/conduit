use std::collections::{BTreeMap, BTreeSet};

use conduit_core::ConfigurationValue;
use conduit_std_catalog::{
    standard_contracts, tick_capability_offer, tick_contract, StandardConfigurationRule,
    StandardKindContract, FILTER_KIND, FORMAT_KIND, GENERIC_VALUE_KIND, MAP_KIND,
    TICK_CONTRACT_REVISION, TICK_IMPLEMENTATION, TICK_VALUE_KIND,
};

const INVENTORY: &str = include_str!("../../../docs/architecture/std-catalog-truth-inventory.tsv");
const HOST_TICK_CONTRACT: &str = include_str!("../../../hosts/std/src/installed_std/contract.rs");
const EXPECTED_HEADER: &str = "kind_id\trevision\tports\tconfiguration\tlimits\tdeclared_terminal_behavior\tinstalled_hosted_implementation\tplanning_binding\tkernel_execution\tcurrent_proof\tbrowser_claim\tpico_claim\tclassification\tstop_line";
const MISDESIGNED: &str = "misdesigned / needs rearticulation";

fn inventory_rows() -> BTreeMap<&'static str, Vec<&'static str>> {
    let mut lines = INVENTORY.lines();
    assert_eq!(lines.next(), Some(EXPECTED_HEADER));

    lines
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(
                fields.len(),
                14,
                "inventory row must retain every required truth field: {line}"
            );
            (fields[0], fields)
        })
        .collect()
}

fn contract_ports(contract: &StandardKindContract) -> String {
    let group = |direction, ports: &[conduit_core::PortDescriptor]| {
        let ports = ports
            .iter()
            .map(|port| format!("{}:{}", port.port_id.as_str(), port.value_kind.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        (!ports.is_empty()).then(|| format!("{direction} {ports}"))
    };
    [
        group("in", &contract.inputs),
        group("out", &contract.outputs),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("; ")
}

fn contract_configuration(contract: &StandardKindContract) -> String {
    if contract.configuration.is_empty() {
        return "none".to_owned();
    }
    contract
        .configuration
        .iter()
        .map(|field| match (&field.default_value, &field.rule) {
            (
                ConfigurationValue::U64(default),
                StandardConfigurationRule::U64Range { minimum, maximum },
            ) => format!(
                "{}:u64[{}..{}]={default}",
                field.key,
                minimum,
                if *maximum == u64::MAX {
                    "MAX".to_owned()
                } else {
                    maximum.to_string()
                }
            ),
            (ConfigurationValue::Bool(default), StandardConfigurationRule::Any) => {
                format!("{}:bool={default}", field.key)
            }
            _ => panic!("inventory formatter must cover every standard configuration field"),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn contract_limits(contract: &StandardKindContract) -> String {
    format!(
        "active={};queue-items={};queue-bytes={}",
        contract.limits.max_active_instances,
        contract.limits.max_queue_items,
        contract.limits.max_queue_bytes
    )
}

#[test]
fn inventory_classifies_every_current_catalog_revision_exactly_once() {
    let rows = inventory_rows();
    let contracts = standard_contracts();
    assert_eq!(rows.len(), contracts.len());

    let mut seen = BTreeSet::new();
    for contract in contracts {
        let kind = contract.kind_id.as_str();
        assert!(
            seen.insert(kind.to_owned()),
            "duplicate catalog kind {kind}"
        );
        let row = rows
            .get(kind)
            .unwrap_or_else(|| panic!("missing truth inventory row for {kind}"));
        let slug = kind.replace('/', "-");
        assert_eq!(row[1], format!("conduit.std/{slug}@1"));
        assert_eq!(row[2], contract_ports(&contract));
        assert_eq!(row[3], contract_configuration(&contract));
        assert_eq!(row[4], contract_limits(&contract));
        assert_eq!(row[5], format!("{:?}", contract.terminal_behavior));
        assert_eq!(row[12], MISDESIGNED);
        assert!(!row[6].is_empty(), "{kind} must name its installed fixture");
        assert!(
            row[7].starts_with("no;"),
            "{kind} must state that fixture self-binding is not honest planning"
        );
        assert!(
            row[8].starts_with("no; conduit-runtime HostRuntime compatibility path only"),
            "{kind} must not be reported as conduit-kernel execution"
        );
        assert!(!row[9].is_empty(), "{kind} must name its actual proof");
        assert!(
            !contract.browser_manifestation_honest && !contract.pico_manifestation_honest,
            "{kind} must not inherit a narrower profile's platform proof"
        );
        assert!(
            !row[13].is_empty(),
            "{kind} must have an explicit stop line"
        );
    }
}

#[test]
fn erased_value_and_numeric_selector_contracts_cannot_appear_proven() {
    let rows = inventory_rows();
    for contract in standard_contracts() {
        let erases_value_kind = contract
            .inputs
            .iter()
            .chain(contract.outputs.iter())
            .any(|port| port.value_kind.as_str() == GENERIC_VALUE_KIND);
        assert!(
            erases_value_kind,
            "audit assumptions changed; reassess inventory"
        );
        assert_eq!(rows[contract.kind_id.as_str()][12], MISDESIGNED);
    }

    for kind in [MAP_KIND, FILTER_KIND, FORMAT_KIND] {
        let configuration = rows[kind][3];
        assert!(configuration.contains("-id:u64"));
        assert_eq!(rows[kind][12], MISDESIGNED);
    }
}

#[test]
fn narrower_kernel_proofs_are_named_without_being_promoted_across_revisions() {
    let rows = inventory_rows();
    for kind in [
        "flow/pulse",
        "presentation/show",
        "flow/filter",
        "flow/tee",
        "time/tick",
        "state/latest",
    ] {
        assert!(
            rows[kind][9].contains("different") || rows[kind][13].contains("across revisions"),
            "{kind} must fence narrower proof from the audited revision"
        );
        assert_eq!(rows[kind][12], MISDESIGNED);
    }
}

#[test]
fn rearticulated_tick_is_distinct_from_the_audited_compatibility_row() {
    let legacy = standard_contracts()
        .into_iter()
        .find(|contract| contract.kind_id.as_str() == "time/tick")
        .expect("legacy tick remains inventoried");
    assert_eq!(legacy.outputs[0].value_kind.as_str(), GENERIC_VALUE_KIND);
    assert_eq!(inventory_rows()["time/tick"][12], MISDESIGNED);

    let tick = tick_contract();
    let offer = tick_capability_offer();
    assert_eq!(tick.outputs[0].value_kind.as_str(), TICK_VALUE_KIND);
    assert_eq!(
        offer.kind_contract_revision.as_str(),
        TICK_CONTRACT_REVISION
    );
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        TICK_IMPLEMENTATION
    );
    assert_eq!(offer.outputs, tick.outputs);
    assert_ne!(
        offer.kind_contract_revision.as_str(),
        inventory_rows()["time/tick"][1]
    );
    for exact_identity in [
        TICK_CONTRACT_REVISION,
        TICK_IMPLEMENTATION,
        TICK_VALUE_KIND,
        conduit_std_catalog::TICK_EXECUTION_PROFILE,
        conduit_std_catalog::TICK_ARTIFACT,
    ] {
        assert!(
            HOST_TICK_CONTRACT.contains(exact_identity),
            "std-host installation must bind exact tick identity {exact_identity}"
        );
    }
}
