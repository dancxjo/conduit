//! Preparation validation of actual tensor shapes against imported symbols.
use crate::{
    imported_onnx::{ImportedOnnxResource, ImportedPortBinding},
    onnx_metadata::{OnnxDimension, OnnxPort},
    ModelPortPresence, ModelTensorIoRefusal,
};
use conduit_data::NamedTensorBundle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedOnnxTensorRefusal {
    Semantic(ModelTensorIoRefusal),
    InvalidBinding,
    SymbolMismatch,
    SymbolBound,
}

impl ImportedOnnxResource {
    pub fn validate_named_inputs(
        &self,
        inputs: &NamedTensorBundle,
    ) -> Result<(), ImportedOnnxTensorRefusal> {
        self.resource()
            .signature()
            .validate_named_inputs(inputs)
            .map_err(ImportedOnnxTensorRefusal::Semantic)?;
        symbols(
            &self.metadata().inputs,
            &self.sidecar().inputs,
            inputs,
            true,
            &mut [None; 256],
            &mut 0,
        )
    }

    /// Outputs must agree with symbols in the actual corresponding inputs,
    /// not merely fall within independently matching dimension envelopes.
    pub fn validate_named_outputs(
        &self,
        inputs: &NamedTensorBundle,
        outputs: &NamedTensorBundle,
    ) -> Result<(), ImportedOnnxTensorRefusal> {
        self.resource()
            .signature()
            .validate_named_inputs(inputs)
            .map_err(ImportedOnnxTensorRefusal::Semantic)?;
        self.resource()
            .signature()
            .validate_named_outputs(outputs)
            .map_err(ImportedOnnxTensorRefusal::Semantic)?;
        let mut values = [None; 256];
        let mut count = 0;
        symbols(
            &self.metadata().inputs,
            &self.sidecar().inputs,
            inputs,
            true,
            &mut values,
            &mut count,
        )?;
        symbols(
            &self.metadata().outputs,
            &self.sidecar().outputs,
            outputs,
            false,
            &mut values,
            &mut count,
        )
    }
}

fn symbols<'a>(
    graph: &'a [OnnxPort],
    bindings: &[ImportedPortBinding],
    tensors: &NamedTensorBundle,
    input: bool,
    values: &mut [Option<(&'a str, u64)>; 256],
    count: &mut usize,
) -> Result<(), ImportedOnnxTensorRefusal> {
    use ImportedOnnxTensorRefusal as R;
    for binding in bindings {
        let port = graph
            .iter()
            .find(|p| p.name == binding.graph_name)
            .ok_or(R::InvalidBinding)?;
        let Some(named) = tensors
            .get()
            .iter()
            .find(|t| t.name == binding.semantic_port)
        else {
            if input && port.presence == ModelPortPresence::Optional {
                continue;
            }
            return Err(R::InvalidBinding);
        };
        let tensor = &named.value;
        if port.dimensions.len() != tensor.dimensions.len() {
            return Err(R::InvalidBinding);
        }
        for (dimension, actual) in port.dimensions.iter().zip(tensor.dimensions.iter()) {
            if let OnnxDimension::Symbol(name) = dimension {
                if let Some((_, previous)) = values[..*count]
                    .iter()
                    .flatten()
                    .find(|(s, _)| *s == name.as_str())
                {
                    if previous != actual {
                        return Err(R::SymbolMismatch);
                    }
                } else {
                    let slot = values.get_mut(*count).ok_or(R::SymbolBound)?;
                    *slot = Some((name, *actual));
                    *count += 1;
                }
            }
        }
    }
    Ok(())
}
