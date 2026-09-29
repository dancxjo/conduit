use crate::prelude::*;
use conduit_core::{
    port_id, CheckedFront, FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, PortDescriptor,
    PortDirection,
};

/// A reviewed, finite family of exact Fores whose inputs are homogeneous.
///
/// This is checker/catalog truth, not a runtime variadic port. Every use is
/// specialized to an ordinary [`CheckedFront`] with a finite number of exact
/// input ports before expansion and planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomogeneousVariadicFore {
    prototype: PortDescriptor,
    output: PortDescriptor,
    minimum_inputs: u16,
    maximum_inputs: u16,
}

impl HomogeneousVariadicFore {
    pub fn new(
        prototype: PortDescriptor,
        output: PortDescriptor,
        minimum_inputs: u16,
        maximum_inputs: u16,
    ) -> Result<Self, String> {
        if prototype.direction != PortDirection::Input {
            return Err("variadic Fore prototype must be an input port".into());
        }
        if output.direction != PortDirection::Output {
            return Err("variadic Fore result must be an output port".into());
        }
        if minimum_inputs == 0 || minimum_inputs > maximum_inputs {
            return Err("variadic Fore input bounds must be finite and non-empty".into());
        }
        Ok(Self {
            prototype,
            output,
            minimum_inputs,
            maximum_inputs,
        })
    }

    pub fn minimum_inputs(&self) -> u16 {
        self.minimum_inputs
    }

    pub fn maximum_inputs(&self) -> u16 {
        self.maximum_inputs
    }

    pub fn specialize(
        &self,
        input_count: usize,
        startup_parameters: Vec<conduit_core::FrontStartupParameter>,
    ) -> Result<CheckedFront, String> {
        let input_count = u16::try_from(input_count)
            .map_err(|_| "variadic Fore input count exceeds its finite bound".to_string())?;
        if input_count < self.minimum_inputs || input_count > self.maximum_inputs {
            return Err(format!(
                "variadic Fore accepts {}..={} inputs, not {input_count}",
                self.minimum_inputs, self.maximum_inputs
            ));
        }
        let width = self.maximum_inputs.to_string().len();
        let inputs = (0..input_count)
            .map(|index| {
                let mut input = self.prototype.clone();
                input.port_id = port_id(&format!(
                    "{}-{:0width$}",
                    self.prototype.port_id.as_str(),
                    usize::from(index) + 1,
                    width = width
                ));
                input
            })
            .collect();
        Ok(CheckedFront::new(
            startup_parameters,
            inputs,
            vec![self.output.clone()],
            None,
        ))
    }

    /// Specializes the complete semantic Kind, including every port-owned law.
    /// The resulting Kind is ordinary exact truth and has no variadic runtime.
    pub fn specialize_kind(&self, template: &Kind, input_count: usize) -> Result<Kind, String> {
        if template.inputs.as_slice() != [self.prototype.clone()]
            || template.outputs.as_slice() != [self.output.clone()]
        {
            return Err("variadic Kind template differs from its reviewed Fore family".into());
        }
        let fore = self.specialize(input_count, template.startup_parameters.clone())?;
        let inputs = fore.inputs().to_vec();
        let mut semantic_laws = Vec::new();
        for law in &template.semantic_laws {
            match law {
                KindSemanticLaw::TerminalTransduction(profile)
                    if profile.input_port_id == self.prototype.port_id =>
                {
                    semantic_laws.extend(inputs.iter().map(|input| {
                        let mut profile = profile.clone();
                        profile.input_port_id = input.port_id.clone();
                        KindSemanticLaw::TerminalTransduction(profile)
                    }));
                }
                KindSemanticLaw::ResourcePorts(contracts) => {
                    let mut specialized = Vec::new();
                    for contract in contracts {
                        if contract.port_id == self.prototype.port_id {
                            specialized.extend(inputs.iter().map(|input| {
                                let mut contract = contract.clone();
                                contract.port_id = input.port_id.clone();
                                contract
                            }));
                        } else {
                            specialized.push(contract.clone());
                        }
                    }
                    specialized.sort_by(|left, right| left.port_id.cmp(&right.port_id));
                    semantic_laws.push(KindSemanticLaw::ResourcePorts(specialized));
                }
                KindSemanticLaw::ValueContracts(contracts) => {
                    let mut specialized = Vec::new();
                    for contract in contracts {
                        match &contract.location {
                            FrontValueLocation::Input(port) if *port == self.prototype.port_id => {
                                specialized.extend(inputs.iter().map(|input| {
                                    let mut contract = contract.clone();
                                    contract.location =
                                        FrontValueLocation::Input(input.port_id.clone());
                                    contract
                                }));
                            }
                            FrontValueLocation::InputAbnormal(port)
                                if *port == self.prototype.port_id =>
                            {
                                specialized.extend(inputs.iter().map(|input| {
                                    let mut contract = contract.clone();
                                    contract.location =
                                        FrontValueLocation::InputAbnormal(input.port_id.clone());
                                    contract
                                }));
                            }
                            _ => specialized.push(contract.clone()),
                        }
                    }
                    specialized.sort();
                    semantic_laws.push(KindSemanticLaw::ValueContracts(specialized));
                }
                _ => semantic_laws.push(law.clone()),
            }
        }
        let specialized = Kind {
            startup_parameters: template.startup_parameters.clone(),
            shorthand: None,
            kind_id: template.kind_id.clone(),
            kind_contract_revision: specialized_kind_identity(
                &template.kind_contract_revision,
                input_count,
            ),
            inputs,
            outputs: fore.outputs().to_vec(),
            configuration: template.configuration.clone(),
            semantic_laws,
            limits: template.limits.clone(),
        };
        specialized
            .validate()
            .map_err(|error| format!("invalid specialized variadic Kind: {error:?}"))?;
        Ok(specialized)
    }
}

pub(crate) fn specialized_kind_identity(base: &KindIdentity, input_count: usize) -> KindIdentity {
    KindIdentity::from(format!("{}/inputs/{input_count}", base.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        kind_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
        NormalCloseTransduction, PortTemporal, TerminalTransductionProfile,
    };

    fn port(name: &str, direction: PortDirection) -> PortDescriptor {
        PortDescriptor {
            port_id: port_id(name),
            value_kind: kind_id("test/value"),
            direction,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }
    }

    #[test]
    fn specialization_is_finite_exact_and_canonically_named() {
        let family = HomogeneousVariadicFore::new(
            port("operand", PortDirection::Input),
            port("result", PortDirection::Output),
            2,
            16,
        )
        .unwrap();
        let fore = family.specialize(3, Vec::new()).unwrap();
        assert_eq!(
            fore.inputs()
                .iter()
                .map(|input| input.port_id.as_str())
                .collect::<Vec<_>>(),
            ["operand-01", "operand-02", "operand-03"]
        );
        assert!(family.specialize(1, Vec::new()).is_err());
        assert!(family.specialize(17, Vec::new()).is_err());
    }

    #[test]
    fn malformed_or_empty_family_refuses_before_catalog_truth() {
        assert!(HomogeneousVariadicFore::new(
            port("operand", PortDirection::Output),
            port("result", PortDirection::Output),
            2,
            16,
        )
        .is_err());
        assert!(HomogeneousVariadicFore::new(
            port("operand", PortDirection::Input),
            port("result", PortDirection::Output),
            0,
            u16::MAX,
        )
        .is_err());
    }

    #[test]
    fn semantic_kind_specialization_repeats_exact_per_input_terminal_law() {
        let mut prototype = port("operand", PortDirection::Input);
        prototype.temporal = PortTemporal::Flow { closes: true };
        prototype.abnormal_kind = Some(kind_id("test/fault"));
        let mut output = port("result", PortDirection::Output);
        output.temporal = PortTemporal::Flow { closes: true };
        output.abnormal_kind = Some(kind_id("test/fault"));
        let family =
            HomogeneousVariadicFore::new(prototype.clone(), output.clone(), 2, 16).unwrap();
        let template = Kind {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: kind_id("flow/merge"),
            kind_contract_revision: KindIdentity::from("flow/merge@1"),
            inputs: vec![prototype],
            outputs: vec![output],
            configuration: Vec::new(),
            semantic_laws: vec![KindSemanticLaw::TerminalTransduction(
                TerminalTransductionProfile {
                    input_port_id: port_id("operand"),
                    output_port_id: port_id("result"),
                    normal_close: NormalCloseTransduction::PropagateAfterDrain,
                    abnormal: AbnormalTerminalTransduction::PropagateAfterDrain,
                    cancellation: CancellationTransduction::NotCancellable,
                },
            )],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 4,
                max_queue_bytes: 1_024,
            },
        };
        let specialized = family.specialize_kind(&template, 3).unwrap();
        assert_eq!(
            specialized.kind_contract_revision.as_str(),
            "flow/merge@1/inputs/3"
        );
        assert_eq!(
            specialized
                .terminal_transductions()
                .map(|profile| profile.input_port_id.as_str())
                .collect::<Vec<_>>(),
            ["operand-01", "operand-02", "operand-03"]
        );
        assert!(specialized.inputs.iter().all(|input| input
            .abnormal_kind
            .as_ref()
            .is_some_and(|kind| kind.as_str() == "test/fault")));
    }
}
