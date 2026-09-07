//! Deterministic combined review of a proposed Body workload against Host offers.
mod queue_plan;

use super::initial_forms::InitialFormSelection;
use conduit_core::{
    BaseImplementationId, CapabilityId, HostAdvertisement, HostId, ResourceClassId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct InitialWorkloadReview {
    pub(super) schema: String,
    pub(super) disposition: String,
    pub(super) selected_form_count: usize,
    pub(super) required_kinds: Vec<String>,
    pub(super) proposed_hosts: Vec<ProposedHost>,
    pub(super) reviewed_realization_count: usize,
    pub(super) body_plan_created: bool,
    pub(super) play_created: bool,
    pub(super) authority_acquired: bool,
    pub(super) resources_acquired: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ProposedHost {
    pub(super) host_id: String,
    pub(super) boot_id: String,
    pub(super) profile_id: String,
    pub(super) offer_generation: u64,
}

pub(super) fn review(
    source: &str,
    selection_json: &str,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
) -> Result<InitialWorkloadReview, String> {
    let selected: Vec<InitialFormSelection> = serde_json::from_str(selection_json)
        .map_err(|_| "initial Form selection is not an exact identity list".to_string())?;
    if selected.len() > conduit_body::MAX_BODY_FORMS {
        return Err("initial Form selection exceeds Body capacity".into());
    }
    let checked_documents = super::initial_forms::check_inventory(source)?;
    let (_, profile) = crate::installed_browser::catalogs()?;
    let backs = crate::installed_browser::backs(
        &{
            let (startup, _) = crate::installed_browser::catalogs()?;
            startup
        },
        &profile,
    )?;
    let mut required_kinds = BTreeSet::new();
    let mut resource_totals = BTreeMap::<(HostId, ResourceClassId), u32>::new();
    let mut capability_totals = BTreeMap::<(HostId, CapabilityId), u32>::new();

    for selected_form in &selected {
        let (checked, form) = checked_documents
            .iter()
            .find_map(|entry| {
                entry
                    .checked
                    .forms
                    .iter()
                    .find(|form| form.name == selected_form.name)
                    .map(|form| (&entry.checked, form))
            })
            .ok_or_else(|| {
                format!(
                    "selected initial Form {:?} is absent from checked inventory",
                    selected_form.name
                )
            })?;
        if selected_form.source_document_id != checked.source_document_id.as_str()
            || selected_form.checked_form_id != form.checked_form_id.as_str()
        {
            return Err(format!(
                "selected initial Form {:?} has a stale or substituted exact identity",
                selected_form.name
            ));
        }
        let expanded =
            conduit_form::expand_canonical_form_with_backs(checked, &form.name, &profile, &backs)
                .map_err(|error| format!("expand reviewed Form {:?}: {error:?}", form.name))?;
        required_kinds.extend(
            expanded
                .gears
                .iter()
                .map(|gear| gear.kind_id.as_str().to_string()),
        );
        let placements =
            conduit_planner::default_expanded_placements(&expanded, hosts).map_err(|error| {
                format!(
                    "initial workload is unrealizable for {:?}: {error}",
                    form.name
                )
            })?;
        queue_plan::review(&expanded, hosts, &placements, bases).map_err(|error| {
            format!(
                "initial workload is unrealizable for {:?}: {error}",
                form.name
            )
        })?;
        accumulate_requirements(
            hosts,
            &placements,
            &mut resource_totals,
            &mut capability_totals,
        )?;
    }
    validate_combined_resources(hosts, &resource_totals)?;
    validate_combined_capabilities(hosts, &capability_totals)?;

    Ok(InitialWorkloadReview {
        schema: "conduit.creche/initial-workload-review@1".into(),
        disposition: "realizable".into(),
        selected_form_count: selected.len(),
        required_kinds: required_kinds.into_iter().collect(),
        proposed_hosts: hosts
            .iter()
            .map(|host| ProposedHost {
                host_id: host.host_id.as_str().into(),
                boot_id: host.boot_id.as_str().into(),
                profile_id: host.profile.as_str().into(),
                offer_generation: host.offer_generation.0,
            })
            .collect(),
        reviewed_realization_count: selected.len(),
        body_plan_created: false,
        play_created: false,
        authority_acquired: false,
        resources_acquired: false,
    })
}

fn accumulate_requirements(
    hosts: &[HostAdvertisement],
    placements: &conduit_planner::PlacementChoices,
    resource_totals: &mut BTreeMap<(HostId, ResourceClassId), u32>,
    capability_totals: &mut BTreeMap<(HostId, CapabilityId), u32>,
) -> Result<(), String> {
    for placement in placements.by_gear.values() {
        let host = hosts
            .iter()
            .find(|host| host.host_id == placement.host_id)
            .ok_or_else(|| "planned Host vanished during combined review".to_string())?;
        let capability = host
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == placement.capability_id)
            .ok_or_else(|| "planned capability vanished during combined review".to_string())?;
        let instances = capability_totals
            .entry((host.host_id.clone(), capability.capability_id.clone()))
            .or_default();
        *instances = instances
            .checked_add(1)
            .ok_or_else(|| "combined capability instance count overflowed".to_string())?;
        for requirement in &capability.resource_requirements {
            let total = resource_totals
                .entry((host.host_id.clone(), requirement.class_id.clone()))
                .or_default();
            *total = total
                .checked_add(requirement.units)
                .ok_or_else(|| "combined resource requirement overflowed".to_string())?;
        }
    }
    Ok(())
}

fn validate_combined_capabilities(
    hosts: &[HostAdvertisement],
    totals: &BTreeMap<(HostId, CapabilityId), u32>,
) -> Result<(), String> {
    for ((host_id, capability_id), required) in totals {
        let host = hosts.iter().find(|host| &host.host_id == host_id).unwrap();
        let capability = host
            .capabilities
            .iter()
            .find(|capability| &capability.capability_id == capability_id)
            .unwrap();
        if *required > u32::from(capability.limits.max_active_instances) {
            return Err(format!(
                "combined initial workload requires {required} active instances of capability {:?} on Host {:?}, above offered limit {}",
                capability_id.as_str(),
                host_id.as_str(),
                capability.limits.max_active_instances,
            ));
        }
    }
    Ok(())
}

fn validate_combined_resources(
    hosts: &[HostAdvertisement],
    totals: &BTreeMap<(HostId, ResourceClassId), u32>,
) -> Result<(), String> {
    for ((host_id, class_id), required) in totals {
        let host = hosts.iter().find(|host| &host.host_id == host_id).unwrap();
        let available: u32 = host
            .resources
            .iter()
            .filter(|resource| &resource.class_id == class_id)
            .try_fold(0_u32, |sum, resource| {
                sum.checked_add(resource.capacity_units)
            })
            .ok_or_else(|| "Host resource capacity overflowed".to_string())?;
        if *required > available {
            return Err(format!(
                "combined initial workload requires {required} units of resource class {:?} on Host {:?}, above offered capacity {available}",
                class_id.as_str(), host_id.as_str()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{BootId, HostId};

    const THREE: &str = concat!(
        include_str!("../../../../../forms/morse-network/main.conduit"),
        "\n",
        include_str!("../../../../../forms/memory-lantern/main.conduit"),
        "\n",
        include_str!("../../../../../forms/desk-telegraph/main.conduit"),
    );

    fn selection(source: &str, names: &[&str]) -> String {
        let inventory = crate::creche::initial_forms::reviewed_inventory(source).unwrap();
        serde_json::to_string(
            &names
                .iter()
                .map(|name| {
                    let form = inventory
                        .forms
                        .iter()
                        .find(|form| form.name == *name)
                        .unwrap();
                    InitialFormSelection {
                        name: form.name.clone(),
                        source_document_id: form.source_document_id.clone(),
                        checked_form_id: form.checked_form_id.clone(),
                    }
                })
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }

    fn browser(host: &str) -> HostAdvertisement {
        crate::installed_browser::advertisement(
            HostId::from(host),
            BootId::from(format!("boot/{host}")),
        )
    }

    #[test]
    fn zero_and_three_forms_have_one_combined_honest_review() {
        for names in [
            Vec::<&str>::new(),
            vec!["morse_network", "memory_lantern", "desk_telegraph"],
        ] {
            let result = review(
                THREE,
                &selection(THREE, &names),
                &[browser("browser/one")],
                &crate::installed_browser::local_bases(),
            )
            .unwrap();
            assert_eq!(result.selected_form_count, names.len());
            assert_eq!(result.reviewed_realization_count, names.len());
            assert!(!result.body_plan_created);
            assert!(!result.play_created);
            assert!(!result.authority_acquired);
            assert!(!result.resources_acquired);
        }
    }

    #[test]
    fn reviewed_bundle_resolves_each_selection_against_its_own_checked_document() {
        let source = serde_json::to_string(&serde_json::json!({
            "schema": "conduit.creche/reviewed-form-bundle@1",
            "forms": [
                { "slug": "morse-network", "source": include_str!("../../../../../forms/morse-network/main.conduit") },
                { "slug": "memory-lantern", "source": include_str!("../../../../../forms/memory-lantern/main.conduit") },
                { "slug": "desk-telegraph", "source": include_str!("../../../../../forms/desk-telegraph/main.conduit") },
            ],
        }))
        .unwrap();
        let result = review(
            &source,
            &selection(
                &source,
                &["morse_network", "memory_lantern", "desk_telegraph"],
            ),
            &[browser("browser/bundle")],
            &crate::installed_browser::local_bases(),
        )
        .unwrap();
        assert_eq!(result.selected_form_count, 3);
        assert_eq!(result.reviewed_realization_count, 3);
    }

    #[test]
    fn missing_capability_and_combined_resource_conflict_are_causal_refusals() {
        let selected = selection(THREE, &["morse_network"]);
        let mut missing = browser("browser/missing");
        missing.capabilities.clear();
        assert!(review(
            THREE,
            &selected,
            &[missing],
            &crate::installed_browser::local_bases(),
        )
        .unwrap_err()
        .contains("unrealizable"));

        let mut scarce = browser("browser/scarce");
        for resource in &mut scarce.resources {
            resource.capacity_units = 1;
        }
        assert!(review(
            THREE,
            &selection(
                THREE,
                &["morse_network", "memory_lantern", "desk_telegraph"],
            ),
            &[scarce],
            &crate::installed_browser::local_bases(),
        )
        .unwrap_err()
        .contains("combined initial workload"));

        let mut instance_limited = browser("browser/instance-limited");
        for capability in &mut instance_limited.capabilities {
            capability.limits.max_active_instances = 1;
        }
        assert!(review(
            THREE,
            &selection(
                THREE,
                &["morse_network", "memory_lantern", "desk_telegraph"],
            ),
            &[instance_limited],
            &crate::installed_browser::local_bases(),
        )
        .unwrap_err()
        .contains("active instances"));
    }

    #[test]
    fn one_body_review_can_select_realizations_across_several_hosts() {
        const DISTRIBUTED: &str = r#"form clock {
    tick: time/every(1s)
}
form note {
    value: text/literal("ready")
}"#;
        let mut clock_host = browser("host/clock");
        clock_host
            .capabilities
            .retain(|offer| offer.kind_id.as_str() == conduit_time::TIME_EVERY_KIND);
        let mut text_host = browser("host/text");
        text_host
            .capabilities
            .retain(|offer| offer.kind_id.as_str() == conduit_text::TEXT_LITERAL_KIND);
        let result = review(
            DISTRIBUTED,
            &selection(DISTRIBUTED, &["clock", "note"]),
            &[clock_host, text_host],
            &crate::installed_browser::local_bases(),
        )
        .unwrap();
        assert_eq!(result.proposed_hosts.len(), 2);
        assert_eq!(result.reviewed_realization_count, 2);
    }
}
