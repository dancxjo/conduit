//! Complete linked descriptions from the same independently pinned Make image.
//! These public immutable facts are not a checker proof, Source custody, native
//! possession, Body membership, an issued grant, or permission to execute.
use crate::{CheckedConnection, CheckedGear, PlotCompletionPolicy, Span};
use conduit_core as c;
#[derive(Clone, Copy)]
pub struct CheckedGearRef<'a> {
    pub gear_id: &'a str,
    pub kind_id: &'a str,
    pub kind_contract_revision: &'a str,
    pub startup_parameters: &'a [c::FrontStartupParameter],
    pub shorthand: Option<(&'a str, &'a str)>,
    pub inputs: c::PortDescriptorRefs<'a>,
    pub outputs: c::PortDescriptorRefs<'a>,
    pub semantic_contract: &'a c::KindSemanticContract,
    pub terminal_transductions: &'a [c::TerminalTransductionProfile],
    pub resource_ports: &'a [c::ResourcePortContract],
    pub configuration: &'a [c::ConfigurationEntry],
    pub pool_references: &'a [c::SharedPoolId],
}
impl<'a> From<&'a CheckedGear> for CheckedGearRef<'a> {
    fn from(v: &'a CheckedGear) -> Self {
        Self {
            gear_id: v.gear_id.as_str(),
            kind_id: v.kind_id.as_str(),
            kind_contract_revision: v.kind_contract_revision.as_str(),
            startup_parameters: &v.startup_parameters,
            shorthand: v.shorthand.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
            inputs: c::PortDescriptorRefs::Owned(&v.inputs),
            outputs: c::PortDescriptorRefs::Owned(&v.outputs),
            semantic_contract: &v.semantic_contract,
            terminal_transductions: &v.terminal_transductions,
            resource_ports: &v.resource_ports,
            configuration: &v.configuration,
            pool_references: &v.pool_references,
        }
    }
}
#[derive(Clone, Copy)]
pub struct CheckedConnectionRef<'a> {
    pub source_gear_id: &'a str,
    pub source_port_id: &'a str,
    pub sink_gear_id: &'a str,
    pub sink_port_id: &'a str,
    pub value_kind: &'a str,
    pub track: c::ConnectionTrack,
    pub temporal: c::PortTemporal,
}
impl<'a> From<&'a CheckedConnection> for CheckedConnectionRef<'a> {
    fn from(v: &'a CheckedConnection) -> Self {
        Self {
            source_gear_id: v.source_gear_id.as_str(),
            source_port_id: v.source_port_id.as_str(),
            sink_gear_id: v.sink_gear_id.as_str(),
            sink_port_id: v.sink_port_id.as_str(),
            value_kind: v.value_kind.as_str(),
            track: v.track,
            temporal: v.temporal,
        }
    }
}
#[derive(Clone, Copy)]
pub struct PlanningOriginRef<'a> {
    pub gear_id: &'a str,
    pub source_span: Span,
}
#[derive(Clone, Copy)]
pub struct LinkedPlanningFactsRef<'a> {
    pub source_document_id: &'a str,
    pub checked_plot_id: &'a str,
    pub expanded_plot_id: &'a str,
    pub name: &'a str,
    pub completion: PlotCompletionPolicy,
    pub gears: &'a [CheckedGearRef<'a>],
    pub connections: &'a [CheckedConnectionRef<'a>],
    pub provenance: &'a [PlanningOriginRef<'a>],
}

impl CheckedGearRef<'_> {
    /// The same callable Front predicate for borrowed and owned descriptions.
    pub fn accepts_callable_front(self, offer: c::CapabilityOfferRef<'_>) -> bool {
        let offered_shorthand =
            offer
                .shorthand
                .or_else(|| match (offer.inputs.get(0), offer.outputs.get(0)) {
                    (Some(input), Some(output))
                        if offer.inputs.len() == 1 && offer.outputs.len() == 1 =>
                    {
                        Some((input.port_id, output.port_id))
                    }
                    _ => None,
                });
        if self.startup_parameters.is_empty()
            && offer.startup_parameters.is_empty()
            && self.semantic_contract.value_contracts().is_empty()
            && offer.semantic_contract.value_contracts().is_empty()
        {
            self.shorthand == offered_shorthand
                && self.inputs.canonical_eq(offer.inputs)
                && self.outputs.canonical_eq(offer.outputs)
        } else {
            let front = |startup: &[c::FrontStartupParameter],
                         inputs: c::PortDescriptorRefs<'_>,
                         outputs: c::PortDescriptorRefs<'_>,
                         shorthand: Option<(&str, &str)>,
                         contract: &c::KindSemanticContract| {
                c::CheckedFront::new(
                    startup.to_vec(),
                    inputs.iter().map(Into::into).collect(),
                    outputs.iter().map(Into::into).collect(),
                    shorthand.map(|(a, b)| (a.into(), b.into())),
                )
                .with_value_contracts(contract.value_contracts().to_vec())
            };
            front(
                self.startup_parameters,
                self.inputs,
                self.outputs,
                self.shorthand,
                self.semantic_contract,
            ) == front(
                offer.startup_parameters,
                offer.inputs,
                offer.outputs,
                offered_shorthand,
                offer.semantic_contract,
            )
        }
    }
    /// Exact law/schema equality with the ordinary mandatory-field placeholder rule.
    pub fn accepts_semantic_contract(self, offer: c::CapabilityOfferRef<'_>) -> bool {
        self.semantic_contract.laws == offer.semantic_contract.laws
            && self.semantic_contract.configuration.len()
                == offer.semantic_contract.configuration.len()
            && self
                .semantic_contract
                .configuration
                .iter()
                .zip(&offer.semantic_contract.configuration)
                .all(|(gear, offered)| {
                    gear.key == offered.key
                        && gear.rule == offered.rule
                        && self
                            .startup_parameters
                            .iter()
                            .find(|p| p.name == gear.key)
                            .is_some_and(|p| {
                                !p.has_default || gear.default_value == offered.default_value
                            })
                })
    }
}
