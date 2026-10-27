//! Exact selected file planning; no ambient RAM data offer is eligible.
use super::*;
use std::collections::BTreeMap;

impl Owner {
    pub(super) fn plan_notebook_file(&mut self, write: bool) -> Result<(), String> {
        let source = crate::plot_source::parse(if write { PUBLISH } else { LOAD })?;
        let plot = source.expand_entry_for_authoring()?;
        let resident = ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        if self.resident.as_ref() != Some(&resident) || self.session.realization().is_some() {
            return Err("notebook file Plan differs from the idle resident Source".into());
        }
        let ad = self.host.advertisement().clone();
        let expected = if write {
            conduit_std_offers::DURABLE_TEXT_SAVE_IMPLEMENTATION
        } else {
            conduit_std_offers::DURABLE_TEXT_LOAD_IMPLEMENTATION
        };
        let offer = ad
            .capabilities
            .iter()
            .find(|offer| offer.implementation.implementation_id.as_str() == expected)
            .ok_or("notebook has no explicitly selected durable file offer")?;
        let [requirement] = offer.authority_requirements.as_slice() else {
            return Err("notebook file offer has unexpected authority requirements".into());
        };
        let grant = AuthorityGrant {
            grant_id: format!(
                "grant/notebook/{}/{}",
                ad.boot_id.as_str(),
                ad.offer_generation.0
            )
            .into(),
            contract_id: requirement.contract_id.clone(),
            host_call_contract_id: requirement.host_call_contract_id.clone(),
            subject_kind: requirement.subject_kind.clone(),
            host_id: ad.host_id.clone(),
            boot_id: ad.boot_id.clone(),
            capability_id: offer.capability_id.clone(),
        };
        let hosts = [ad.clone()];
        let placements =
            conduit_planner::default_expanded_placements(&plot.expanded, &hosts).map_err(debug)?;
        let rows = if write {
            [
                (PortDirection::Input, "value", 4096),
                (PortDirection::Output, "saved", 116),
                (PortDirection::Output, "failed", 1),
            ]
        } else {
            [
                (PortDirection::Input, "selected", 116),
                (PortDirection::Output, "value", 4096),
                (PortDirection::Output, "failed", 1),
            ]
        };
        let boundaries = rows
            .into_iter()
            .map(|(direction, name, byte_capacity)| {
                (
                    conduit_planner::ForeBoundaryKey {
                        direction,
                        front_port_id: port_id(name),
                        track: if name == "failed" {
                            ConnectionTrack::AbnormalTerminal
                        } else {
                            ConnectionTrack::Payload
                        },
                    },
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity,
                    },
                )
            })
            .collect();
        let plan = conduit_planner::plan_expanded_authoring_with_activations(
            &source.check()?,
            &plot,
            source.authoring_catalog(),
            &conduit_plot::CanonicalBackCatalog::new(),
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[grant],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .map_err(debug)?;
        self.session
            .propose(
                vec![conduit_body::BodyPlotPlan {
                    plot: resident,
                    plan,
                }],
                &ad.host_id,
                &ad.boot_id,
            )
            .map_err(debug)?;
        Ok(())
    }
}
