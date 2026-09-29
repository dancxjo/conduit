use crate::prelude::*;
use conduit_core::{port_id, CheckedFront, PortDescriptor, PortDirection};

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, PortTemporal};

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
}
